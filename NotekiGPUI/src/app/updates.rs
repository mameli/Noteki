//! Updates through Sparkle (ADR 0011), as the Swift app had them: at launch
//! and once a day, when left on in Settings, Sparkle reads the appcast of the
//! latest GitHub Release (the request carries nothing about the user or the
//! Vault), and shows its window with what is new and *Install Update*: it
//! downloads the disk image, checks its EdDSA signature and Developer ID, and
//! opens the new version. Sparkle.framework is in the bundle, loaded at launch
//! (scripts/bundle.sh); run from `cargo` there is none, and no updates.

#[cfg(target_os = "macos")]
mod sparkle {
    use std::cell::OnceCell;

    use objc2::msg_send;
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::{NSBundle, NSString};

    thread_local! {
        /// The SPUStandardUpdaterController, once loaded (main thread only).
        static CONTROLLER: OnceCell<Option<Retained<AnyObject>>> = const { OnceCell::new() };
    }

    fn load() -> Option<Retained<AnyObject>> {
        let frameworks = NSBundle::mainBundle().privateFrameworksPath()?;
        let path = NSString::from_str(&format!("{frameworks}/Sparkle.framework"));
        let bundle = NSBundle::bundleWithPath(&path)?;
        // SAFETY: Sparkle's code, signed with the app's, as the bundle ships it.
        if !unsafe { bundle.load() } {
            return None;
        }
        let class = AnyClass::get(c"SPUStandardUpdaterController")?;
        let none: Option<&AnyObject> = None;
        // SAFETY: Sparkle's documented initializer; nil delegates are allowed.
        unsafe {
            let allocated: Allocated<AnyObject> = msg_send![class, alloc];
            msg_send![allocated, initWithStartingUpdater: true, updaterDelegate: none, userDriverDelegate: none]
        }
    }

    pub fn with<R>(f: impl FnOnce(&AnyObject) -> R) -> Option<R> {
        CONTROLLER.with(|c| c.get_or_init(load).as_deref().map(f))
    }

    fn updater(controller: &AnyObject) -> Retained<AnyObject> {
        // SAFETY: `updater` is a non-null property of the controller.
        unsafe { msg_send![controller, updater] }
    }

    pub fn check(controller: &AnyObject) {
        let none: Option<&AnyObject> = None;
        // SAFETY: an IBAction taking the sender, which may be nil.
        unsafe {
            let _: () = msg_send![controller, checkForUpdates: none];
        }
    }

    pub fn automatic(controller: &AnyObject) -> bool {
        // SAFETY: a BOOL property of SPUUpdater.
        unsafe { msg_send![&*updater(controller), automaticallyChecksForUpdates] }
    }

    pub fn set_automatic(controller: &AnyObject, on: bool) {
        // SAFETY: the setter of that property (kept by Sparkle in the defaults).
        unsafe {
            let _: () = msg_send![&*updater(controller), setAutomaticallyChecksForUpdates: on];
        }
    }
}

/// Loads Sparkle and starts its daily check: at launch, once.
pub fn start() {
    #[cfg(target_os = "macos")]
    sparkle::with(|_| ());
}

/// Whether this Noteki can update itself (it runs from its bundle).
pub fn available() -> bool {
    #[cfg(target_os = "macos")]
    return sparkle::with(|_| ()).is_some();
    #[cfg(not(target_os = "macos"))]
    false
}

/// Sparkle's window: the new version and *Install Update*, or "You're up to date".
pub fn check() {
    #[cfg(target_os = "macos")]
    sparkle::with(sparkle::check);
}

/// Whether Sparkle looks for a new version by itself, every day.
pub fn checks_automatically() -> bool {
    #[cfg(target_os = "macos")]
    return sparkle::with(sparkle::automatic).unwrap_or(false);
    #[cfg(not(target_os = "macos"))]
    false
}

pub fn set_checks_automatically(on: bool) {
    #[cfg(target_os = "macos")]
    sparkle::with(|c| sparkle::set_automatic(c, on));
    #[cfg(not(target_os = "macos"))]
    let _ = on;
}
