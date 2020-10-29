use rand::distributions::Alphanumeric;
use rand::prelude::*;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::iter;
use std::mem;
use std::path::{Path, PathBuf};

pub struct OverlayDirectory {
    pub base_path: PathBuf,
    pub run_id: OsString,
}

impl OverlayDirectory {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Result<Self, io::Error> {
        let base_path = base_path.as_ref().to_path_buf();
        let mut rng = thread_rng();
        let run_id: String = iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .take(8)
            .collect();

        fs::create_dir_all(base_path.clone())?;
        Ok(Self {
            base_path,
            run_id: OsString::from(run_id),
        })
    }

    fn create_folder_for_file<P: AsRef<Path>>(path: P) -> Result<(), io::Error> {
        let directory = path.as_ref().with_file_name("");
        fs::create_dir_all(directory)?;
        Ok(())
    }

    pub fn create_file_for_write<P: AsRef<Path>>(&self, path: P) -> Result<OverlayFile, io::Error> {
        let path = self.base_path.join(path);
        Self::create_folder_for_file(&path)?;
        OverlayFile::create_for_write(path, self.run_id.clone())
    }

    pub fn create<P: AsRef<Path>>(
        &self,
        path: P,
        options: OpenOptions,
    ) -> Result<OverlayFile, io::Error> {
        let path = self.base_path.join(path);
        Self::create_folder_for_file(&path)?;
        OverlayFile::create(path, self.run_id.clone(), options)
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
    pub fn create_for_write<P: AsRef<Path>>(path: P, run_id: OsString) -> Result<Self, io::Error> {
        let mut options = OpenOptions::new();
        options.write(true).read(true);
        Self::create(path, run_id, options)
    }

    pub fn create<P: AsRef<Path>>(
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

        let file = options.open(tmp_path.clone())?;
        Ok(Self {
            path,
            file: Some(file),
            tmp_path,
            run_id,
        })
    }

    pub fn commit(mut self) -> Result<(), io::Error> {
        mem::drop(self.file.take().unwrap());
        fs::rename(self.tmp_path.clone(), self.path.clone())?;
        Ok(())
    }
}

impl Drop for OverlayFile {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            drop(file);
            fs::remove_file(&self.tmp_path).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    #[test]
    fn test_overlay_file_create() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let overlay_file = tmp_dir.path().join("test.bin");
        let file = OverlayFile::create_for_write(overlay_file, "".into()).unwrap();
        assert!(tmp_dir.path().join("test.bin.tmp").exists());
        file.commit().unwrap();
        assert!(!tmp_dir.path().join("test.bin.tmp").exists());
        assert!(tmp_dir.path().join("test.bin").exists());
    }

    #[test]
    fn test_overlay_file_run_id() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let overlay_file = tmp_dir.path().join("test.bin");
        let file = OverlayFile::create_for_write(overlay_file, "2333".into()).unwrap();
        assert!(tmp_dir.path().join("test.bin.2333.tmp").exists());
        file.commit().unwrap();
        assert!(!tmp_dir.path().join("test.bin.2333.tmp").exists());
        assert!(tmp_dir.path().join("test.bin").exists());
    }

    #[test]
    fn test_overlay_file_write_twice() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let overlay_file = tmp_dir.path().join("test.bin");
        OverlayFile::create_for_write(overlay_file.clone(), "".into())
            .unwrap()
            .commit()
            .unwrap();
        OverlayFile::create_for_write(overlay_file.clone(), "".into())
            .unwrap()
            .commit()
            .unwrap();
    }

    #[test]
    fn test_overlay_file_create_twice() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let overlay_file = tmp_dir.path().join("test.bin");
        let file1 = OverlayFile::create_for_write(overlay_file.clone(), "".into()).unwrap();
        assert!(OverlayFile::create_for_write(overlay_file.clone(), "".into()).is_err());
        drop(file1);
    }

    #[test]
    fn test_overlay_file_drop() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let overlay_file = tmp_dir.path().join("test.bin");
        let file1 = OverlayFile::create_for_write(overlay_file.clone(), "".into()).unwrap();
        drop(file1);
        assert!(!overlay_file.exists());
        assert!(!tmp_dir.path().join("test.bin.tmp").exists());
    }

    #[test]
    fn test_overlay_directory_create() {
        let tmp_dir = TempDir::new("overlay").unwrap();
        let directory = OverlayDirectory::new(tmp_dir.path().join("test")).unwrap();
        let dir = tmp_dir.path().join("test");
        assert!(dir.is_dir());
        assert!(dir.exists());
        let file = directory.create_file_for_write("233/2333/233333.zip").unwrap();
        file.commit().unwrap();
        let file_path = tmp_dir.path().join("test/233/2333/233333.zip");
        assert!(file_path.exists());
    }
}
