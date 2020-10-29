use futures_util::StreamExt;
use overlay::OverlayFile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn download_to_file(
    url: String,
    file: &mut OverlayFile,
) -> Result<usize, Box<dyn std::error::Error>> {
    let response = reqwest::get(url.as_str()).await?;
    let mut stream = response.bytes_stream();
    let mut size = 0;
    while let Some(v) = stream.next().await {
        let v = v?;
        file.file().write_all(&v).await?;
        size += v.len();
    }
    Ok(size)
}

pub async fn content_of(
    url: String,
    file: &mut OverlayFile,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let size = download_to_file(url, file).await?;
    let mut buf = Vec::with_capacity(size);
    let file = file.file();
    file.seek(std::io::SeekFrom::Start(0)).await?;
    file.read_to_end(&mut buf).await?;
    assert_eq!(buf.len(), size);
    Ok(buf)
}
