use super::*;

pub(super) fn repository_gate_route() -> Route {
    Route::RepositoryGate(RepositoryGateState {
        message: "Checking repository and syncing latest changes".to_owned(),
    })
}

pub(super) fn blocking_failure_route() -> Route {
    Route::BlockingFailure(FailureState {
        title: "Repository blocked".to_owned(),
        message: "pulling latest budget data failed: remote rejected update".to_owned(),
        retry: RetryTarget::RepositoryGate,
    })
}
