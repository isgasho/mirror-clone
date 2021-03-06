use rand::distributions::Alphanumeric;
use rand::prelude::*;
use std::ffi::OsString;
use std::io;
use std::iter;
use std::mem;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File, OpenOptions};

pub struct OverlayDirectory {
    pub base_path: PathBuf,
    pub run_id: OsString,
}

impl OverlayDirectory {
    pub async fn new<P: AsRef<Path>>(base_path: P) -> Result<Self, io::Error> {
        let base_path = base_path.as_ref().to_path_buf();
        let mut rng = thread_rng();
        let run_id: String = iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .take(8)
            .collect();

        fs::create_dir_all(base_path.clone()).await?;
        Ok(Self {
            base_path,
            run_id: OsString::from(run_id),
        })
    }

    async fn create_folder_for_file<P: AsRef<Path>>(path: P) -> Result<(), io::Error> {
        let directory = path.as_ref().with_file_name("");
        fs::create_dir_all(directory).await?;
        Ok(())
    }

    pub async fn create_file_for_write<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<OverlayFile, io::Error> {
        let path = self.base_path.join(path);
        Self::create_folder_for_file(&path).await?;
        OverlayFile::create_for_write(path, self.run_id.clone()).await
    }

    pub async fn create<P: AsRef<Path>>(
        &self,
        path: P,
        options: OpenOptions,
    ) -> Result<OverlayFile, io::Error> {
        let path = self.base_path.join(path);
        Self::create_folder_for_file(&path).await?;
        OverlayFile::create(path, self.run_id.clone(), options).await
    }
}

pub struct OverlayFile {
    pub tmp_path: PathBuf,
    pub run_id: OsString,
    pub path: PathBuf,
    pub file: Option<File>,
}

const TMP_FILE_SUFFIX: &'static str = ".tmp";

impl OverlayFile {
    pub async fn create_for_write<P: AsRef<Path>>(
        path: P,
        run_id: OsString,
    ) -> Result<Self, io::Error> {
        let mut options = OpenOptions::new();
        options.write(true).read(true);
        Self::create(path, run_id, options).await
    }

    pub async fn create<P: AsRef<Path>>(
        path: P,
        run_id: OsString,
        mut options: OpenOptions,
    ) -> Result<Self, io::Error> {
        let path = path.as_ref().to_path_buf();
        options.create_new(true).truncate(true);
        let mut tmp_path = path.clone().as_os_str().to_owned();
        if !run_id.is_empty() {
            tmp_path.push(".");
            tmp_path.push(run_id.clone());
        }
        tmp_path.push(TMP_FILE_SUFFIX);

        let tmp_path = PathBuf::from(tmp_path);

        let file = options.open(tmp_path.clone()).await?;
        Ok(Self {
            path,
            file: Some(file),
            tmp_path,
            run_id,
        })
    }

    pub async fn commit(mut self) -> Result<(), io::Error> {
        mem::drop(self.file.take().unwrap());
        fs::rename(self.tmp_path.clone(), self.path.clone()).await?;
        Ok(())
    }

    pub fn file(&mut self) -> &mut File {
        self.file.as_mut().unwrap()
    }
}

impl Drop for OverlayFile {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            drop(file);
            std::fs::remove_file(&self.tmp_path).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    #[tokio::test]
    async fn test_overlay_file_create() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let overlay_file = tmp_dir.path().join("test.bin");
        let file = OverlayFile::create_for_write(overlay_file, "".into())
            .await
            .unwrap();
        assert!(tmp_dir.path().join("test.bin.tmp").exists());
        file.commit().await.unwrap();
        assert!(!tmp_dir.path().join("test.bin.tmp").exists());
        assert!(tmp_dir.path().join("test.bin").exists());
    }

    #[tokio::test]
    async fn test_overlay_file_run_id() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let overlay_file = tmp_dir.path().join("test.bin");
        let file = OverlayFile::create_for_write(overlay_file, "2333".into())
            .await
            .unwrap();
        assert!(tmp_dir.path().join("test.bin.2333.tmp").exists());
        file.commit().await.unwrap();
        assert!(!tmp_dir.path().join("test.bin.2333.tmp").exists());
        assert!(tmp_dir.path().join("test.bin").exists());
    }

    #[tokio::test]
    async fn test_overlay_file_write_twice() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let overlay_file = tmp_dir.path().join("test.bin");
        OverlayFile::create_for_write(overlay_file.clone(), "".into())
            .await
            .unwrap()
            .commit()
            .await
            .unwrap();
        OverlayFile::create_for_write(overlay_file.clone(), "".into())
            .await
            .unwrap()
            .commit()
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_overlay_file_create_twice() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let overlay_file = tmp_dir.path().join("test.bin");
        let file1 = OverlayFile::create_for_write(overlay_file.clone(), "".into())
            .await
            .unwrap();
        assert!(
            OverlayFile::create_for_write(overlay_file.clone(), "".into())
                .await
                .is_err()
        );
        drop(file1);
    }

    #[tokio::test]
    async fn test_overlay_file_drop() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let overlay_file = tmp_dir.path().join("test.bin");
        let file1 = OverlayFile::create_for_write(overlay_file.clone(), "".into())
            .await
            .unwrap();
        drop(file1);
        assert!(!overlay_file.exists());
        assert!(!tmp_dir.path().join("test.bin.tmp").exists());
    }


    #[tokio::test]
    async fn test_overlay_file_drop_retain() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let tmp_dir = TempDir::new("overlay").unwrap();
        let overlay_file = tmp_dir.path().join("test.bin");
        let mut f = File::create(&overlay_file).await.unwrap();
        f.write_all(b"2333333").await.unwrap();
        drop(f);
        let file1 = OverlayFile::create_for_write(overlay_file.clone(), "".into())
            .await
            .unwrap();
        drop(file1);
        assert!(overlay_file.exists());
        assert!(!tmp_dir.path().join("test.bin.tmp").exists());
        let mut f = File::open(&overlay_file).await.unwrap();
        let mut buf = String::new();
        f.read_to_string(&mut buf).await.unwrap();
        assert_eq!(buf.as_bytes(), b"2333333");
    }

    #[tokio::test]
    async fn test_overlay_directory_create() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let directory = OverlayDirectory::new(tmp_dir.path().join("test"))
            .await
            .unwrap();
        let dir = tmp_dir.path().join("test");
        assert!(dir.is_dir());
        assert!(dir.exists());
        let file = directory
            .create_file_for_write("233/2333/233333.zip")
            .await
            .unwrap();
        file.commit().await.unwrap();
        let file_path = tmp_dir.path().join("test/233/2333/233333.zip");
        assert!(file_path.exists());
    }
}
