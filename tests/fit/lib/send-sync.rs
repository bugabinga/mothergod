fn is_send_sync<T: Send + Sync>() {}

pub fn idiom() {
    is_send_sync::<mothergod::Error>();
    is_send_sync::<mothergod::WriteError>();
    is_send_sync::<mothergod::Method>();
}
