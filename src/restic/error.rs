#[derive(Debug)]
pub enum ResticError {
    /// Return Code 1 - fatal error (no snapshot created)
    Fatal,
    /// Return Code 3 - some source data could not be read (incomplete snapshot created)
    Incomplete,
    /// Return Code 10 - repository does not exist
    RepositoryUnavailable,
    /// Return Code 11 - repository is already locked
    RepositoryLocked,
    /// Return Code 12 - incorrect password
    IncorrectPassword,
}

impl std::fmt::Display for ResticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ResticError::Fatal => "Fatal Error (no snapshot created)",
            ResticError::Incomplete => {
                "some source data could not be read (incomplete snapshot created)"
            }
            ResticError::RepositoryUnavailable => "repository does not exist",
            ResticError::RepositoryLocked => "repository is already locked",
            ResticError::IncorrectPassword => "incorrect password",
        })
    }
}

impl cmdbind::errors::FromExitCode for ResticError {
    fn from_code(code: i32) -> Option<Self>
    where
        Self: Sized,
    {
        match code {
            1 => Some(Self::Fatal),
            3 => Some(Self::Incomplete),
            10 => Some(Self::RepositoryUnavailable),
            11 => Some(Self::RepositoryLocked),
            12 => Some(Self::IncorrectPassword),
            _ => None,
        }
    }
}
