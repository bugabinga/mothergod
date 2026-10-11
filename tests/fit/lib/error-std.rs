fn is_error<E: std::error::Error + std::fmt::Display + std::fmt::Debug>() {}

pub fn idiom() {
    is_error::<mothergod::Error>();
    is_error::<mothergod::WriteError>();
}
