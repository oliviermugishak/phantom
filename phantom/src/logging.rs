use std::sync::OnceLock;

fn env_truthy(key: &str) -> bool {
    matches!(
        std::env::var(key).ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

pub fn trace_detail_enabled() -> bool {
    static TRACE_DETAIL: OnceLock<bool> = OnceLock::new();
    *TRACE_DETAIL.get_or_init(|| env_truthy("PHANTOM_TRACE_DETAIL"))
}
