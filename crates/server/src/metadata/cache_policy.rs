//! Refresh jobs bypass every response cache, including blocking TMDB workers.
use std::cell::Cell;
use std::future::Future;

tokio::task_local! { static FRESH_FETCH: bool; }
thread_local! { static BLOCKING_FRESH_FETCH: Cell<bool> = const { Cell::new(false) }; }

pub(crate) fn bypassed() -> bool {
    FRESH_FETCH.try_with(|value| *value).unwrap_or(false) || BLOCKING_FRESH_FETCH.get()
}

pub(crate) async fn fresh<F: Future>(future: F) -> F::Output {
    FRESH_FETCH.scope(true, future).await
}

pub(crate) fn blocking<R>(
    fresh: bool,
    operation: impl FnOnce() -> R,
) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BLOCKING_FRESH_FETCH.set(self.0);
        }
    }
    let _restore = Restore(BLOCKING_FRESH_FETCH.replace(fresh));
    operation()
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn refresh_policy_is_scoped_and_does_not_leak_to_unrelated_tasks() {
        assert!(!super::bypassed());
        super::fresh(async {
            assert!(super::bypassed());
            tokio::spawn(async {
                assert!(!super::bypassed());
            })
            .await
            .unwrap();
        })
        .await;
        assert!(!super::bypassed());
        super::blocking(true, || assert!(super::bypassed()));
        assert!(!super::bypassed());
    }
}
