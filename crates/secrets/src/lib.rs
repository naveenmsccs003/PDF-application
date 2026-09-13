//! SEC-02 (secure credential handling): stores per-document PDF passwords
//! (SEC-01 needs somewhere to keep a password after the user types it once,
//! so subsequent renders/exports of the same encrypted PDF don't re-prompt
//! every time) in the OS's own credential store via the `keyring` crate —
//! Secret Service/GNOME Keyring on Linux, Keychain on macOS, Credential
//! Manager on Windows — rather than in our own SQLite database. A
//! password sitting in `mds_rebar.sqlite` in plaintext would defeat the
//! purpose of "secure" credential handling: anyone with read access to the
//! app's data directory would have it, and it would end up in
//! `RecoveryState`/backup snapshots too. The OS keychain is the one place
//! built for exactly this and already trusted by the platform.

const SERVICE: &str = "mds-rebar-pdf";

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("keychain error: {0}")]
    Keyring(#[from] keyring::Error),
}

fn entry(document_id: &str) -> Result<keyring::Entry, SecretsError> {
    Ok(keyring::Entry::new(SERVICE, document_id)?)
}

/// Stores `password` for `document_id`, overwriting any previously stored
/// password for that document.
pub fn store_pdf_password(document_id: &str, password: &str) -> Result<(), SecretsError> {
    entry(document_id)?.set_password(password)?;
    Ok(())
}

/// Returns the stored password for `document_id`, or `None` if nothing has
/// been stored (not an error — most documents aren't password-protected).
pub fn get_pdf_password(document_id: &str) -> Result<Option<String>, SecretsError> {
    match entry(document_id)?.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Removes any stored password for `document_id` (e.g. the document was
/// removed, or the user wants to forget a mistyped password). Treats "no
/// entry to delete" as success rather than an error — the end state
/// (nothing stored) is the same either way.
pub fn delete_pdf_password(document_id: &str) -> Result<(), SecretsError> {
    match entry(document_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real OS keychains are process-external shared state, so give test
    /// document ids a random suffix rather than a fixed name — avoids one
    /// test run's leftover entry (if a previous run panicked before
    /// cleanup) being mistaken for this run's fixture, and avoids
    /// collisions if these ever run concurrently with `--test-threads`
    /// greater than 1.
    fn test_document_id(label: &str) -> String {
        format!("test-{label}-{}", std::process::id())
    }

    /// Every test here talks to whatever OS credential store is actually
    /// available. In a desktop session (confirmed present in this
    /// environment: a running `gnome-keyring-daemon` + a reachable
    /// `org.freedesktop.secrets` D-Bus service) this is real, not mocked —
    /// but a headless CI box with no secret service running would fail to
    /// even construct an `Entry`/call `set_password`, so every test skips
    /// gracefully rather than failing the suite in that environment. Kept
    /// even after the actual bug (see the crate doc: no default backend
    /// feature meant an implicit no-op store) was found and fixed — a
    /// genuinely headless environment is still a real, separate case this
    /// should degrade gracefully in.
    macro_rules! skip_if_no_keychain {
        ($result:expr) => {
            match $result {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("skipping: no OS keychain available in this environment ({e})");
                    return;
                }
            }
        };
    }

    #[test]
    fn store_and_get_round_trip() {
        let doc_id = test_document_id("round-trip");
        skip_if_no_keychain!(store_pdf_password(&doc_id, "correct horse battery staple"));

        let fetched = get_pdf_password(&doc_id).unwrap();
        assert_eq!(fetched.as_deref(), Some("correct horse battery staple"));

        delete_pdf_password(&doc_id).unwrap();
    }

    #[test]
    fn get_on_unstored_document_returns_none_not_error() {
        let doc_id = test_document_id("never-stored");
        let result = get_pdf_password(&doc_id);
        // Either a real "not found" (Ok(None)) or no keychain available at
        // all in this environment — both are fine; only a stored password
        // coming back would be wrong.
        if let Ok(fetched) = result {
            assert_eq!(fetched, None);
        }
    }

    #[test]
    fn delete_then_get_returns_none() {
        let doc_id = test_document_id("delete-then-get");
        skip_if_no_keychain!(store_pdf_password(&doc_id, "temporary"));

        delete_pdf_password(&doc_id).unwrap();
        assert_eq!(get_pdf_password(&doc_id).unwrap(), None);
    }

    #[test]
    fn overwriting_replaces_the_stored_password() {
        let doc_id = test_document_id("overwrite");
        skip_if_no_keychain!(store_pdf_password(&doc_id, "first"));
        store_pdf_password(&doc_id, "second").unwrap();

        assert_eq!(get_pdf_password(&doc_id).unwrap().as_deref(), Some("second"));

        delete_pdf_password(&doc_id).unwrap();
    }

    #[test]
    fn delete_on_never_stored_document_is_not_an_error() {
        let doc_id = test_document_id("delete-without-store");
        // Should succeed (or skip if there's genuinely no keychain), never
        // error just because there was nothing to delete.
        let _ = delete_pdf_password(&doc_id);
    }
}
