use thiserror::Error;

#[derive(Debug, Error)]
#[error("{0}")]
pub enum InitError {
	GitInit(git2::Error),
	WriteConfig(std::io::Error),
}

#[derive(Debug, Error)]
#[error("{0}")]
pub enum OpenError {
	GitOpen(git2::Error),
}

#[derive(Debug, Error)]
#[error("{0}")]
pub enum AddCrateError {
	CommitFailed(git2::Error),
}

#[derive(Debug, Error)]
#[error("{0}")]
pub enum YankCrateError {
	CommitFailed(git2::Error),
}
