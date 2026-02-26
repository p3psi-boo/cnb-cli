use anyhow::{Result, bail};
use std::future::Future;

pub const MAX_AUTO_PAGES: u32 = 1000;
pub const DEFAULT_REPO_PAGE_SIZE: u32 = 10;
pub const DEFAULT_LIST_PAGE_SIZE: u32 = 30;

pub async fn collect_all_pages<T, F, Fut>(
    page: Option<u32>,
    page_size: Option<u32>,
    default_page_size: u32,
    mut fetch: F,
) -> Result<Vec<T>>
where
    F: FnMut(u32, u32) -> Fut,
    Fut: Future<Output = Result<Vec<T>>>,
{
    let mut collected = Vec::new();
    let mut p = page.unwrap_or(1);
    let ps = page_size.unwrap_or(default_page_size);

    for _ in 0..MAX_AUTO_PAGES {
        let batch = fetch(p, ps).await?;
        if batch.is_empty() {
            return Ok(collected);
        }

        let done = batch.len() < ps as usize;
        collected.extend(batch);
        if done {
            return Ok(collected);
        }

        p += 1;
    }

    bail!("pagination exceeded {MAX_AUTO_PAGES} pages; aborting")
}
