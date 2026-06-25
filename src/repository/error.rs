use std::fmt::{Debug, Display, Formatter};

#[derive(Debug)]
pub enum Error {
    ReDbTable { message: String, cause: redb::TableError },
    ReDbStorage { message: String, cause: redb::StorageError },
    ReDbCommit { message: String, cause: redb::CommitError },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <Self as Debug>::fmt(self, f)
    }
}

impl std::error::Error for Error {}
