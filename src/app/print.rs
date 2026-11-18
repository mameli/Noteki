//! *Export as PDF…* and *Print…* (SPEC › Export): the page *Export as HTML…*
//! makes, laid out by the system's WebKit in a window never shown, and paged
//! by AppKit's printing on the Mac's paper: to a PDF file, or through the
//! print panel. WebKit is the system's: nothing is added to the app, and it
//! is loaded only while a page is printed.

use std::path::PathBuf;

/// What is done with the page.
pub enum Job {
    /// The print panel, as a sheet of the window in front.
    Print,
    /// A PDF at this path, paged as a print is.
    Pdf(PathBuf),
}

/// Lays `html` out and runs `job`; `done` is called on the main thread with
/// what happened (a print cancelled in its panel is `Ok`).
pub fn run(html: &str, title: &str, job: Job, done: impl FnOnce(Result<(), String>) + 'static) {
    #[cfg(target_os = "macos")]
    mac::run(html, title, job, Box::new(done));
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (html, title, job);
        done(Err("printing is only on the Mac".into()));
    }
}

/// Whether this system can print and export PDFs.
pub const AVAILABLE: bool = cfg!(target_os = "macos");

#[cfg(target_os = "macos")]
mod mac {
    use std::cell::RefCell;

    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSApplication, NSBackingStoreType, NSPrintInfo, NSPrintJobSavingURL, NSPrintSaveJob,
        NSPrintingPaginationMode, NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::{
        NSError, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL,
    };
    use objc2_web_kit::{WKNavigation, WKNavigationDelegate, WKWebView, WKWebViewConfiguration};

    use super::Job;

    type Done = Box<dyn FnOnce(Result<(), String>)>;

    /// A page being printed: kept alive until its job is over.
    struct Printing {
        window: Retained<NSWindow>,
        web: Retained<WKWebView>,
        _delegate: Retained<Loaded>,
        op: Option<Retained<objc2_app_kit::NSPrintOperation>>,
        job: Job,
        title: String,
        done: Done,
    }

    thread_local! {
        static PRINTING: RefCell<Vec<Printing>> = const { RefCell::new(Vec::new()) };
    }

    pub struct LoadedIvars {
        id: usize,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements; Loaded has no Drop.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[ivars = LoadedIvars]
        struct Loaded;

        unsafe impl NSObjectProtocol for Loaded {}

        impl Loaded {
            /// The print operation's end, as its `didRunSelector`.
            #[unsafe(method(printOperationDidRun:success:contextInfo:))]
            fn did_run(
                &self,
                _op: &objc2_app_kit::NSPrintOperation,
                _success: bool,
                _context: *mut std::ffi::c_void,
            ) {
                // Called on the print's own thread: back to the main one.
                let id = self.ivars().id;
                dispatch2::DispatchQueue::main().exec_async(move || finish(id, Ok(())));
            }
        }

        unsafe impl WKNavigationDelegate for Loaded {
            #[unsafe(method(webView:didFinishNavigation:))]
            fn did_finish(&self, _web: &WKWebView, _nav: Option<&WKNavigation>) {
                let id = self.ivars().id;
                // Out of WebKit's callback: the print runs a modal loop.
                let mtm = self.mtm();
                dispatch_main(mtm, move || print(id, Ok(())));
            }

            #[unsafe(method(webView:didFailNavigation:withError:))]
            fn did_fail(&self, _web: &WKWebView, _nav: Option<&WKNavigation>, error: &NSError) {
                let id = self.ivars().id;
                let message = error.localizedDescription().to_string();
                dispatch_main(self.mtm(), move || print(id, Err(message)));
            }

            #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
            fn did_fail_early(
                &self,
                _web: &WKWebView,
                _nav: Option<&WKNavigation>,
                error: &NSError,
            ) {
                let id = self.ivars().id;
                let message = error.localizedDescription().to_string();
                dispatch_main(self.mtm(), move || print(id, Err(message)));
            }
        }
    );

    /// Runs `f` on the main queue, after the current callback returns.
    fn dispatch_main(_: MainThreadMarker, f: impl FnOnce() + 'static) {
        use std::sync::Mutex;
        // The main queue only ever runs it on the main thread, where it was made.
        struct MainOnly(Mutex<Option<Box<dyn FnOnce()>>>);
        unsafe impl Send for MainOnly {}
        unsafe impl Sync for MainOnly {}
        let cell = MainOnly(Mutex::new(Some(Box::new(f))));
        dispatch2::DispatchQueue::main().exec_async(move || {
            let cell = &cell;
            if let Some(f) = cell.0.lock().ok().and_then(|mut f| f.take()) {
                f();
            }
        });
    }

    fn next_id() -> usize {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }

    pub fn run(html: &str, title: &str, job: Job, done: Done) {
        let Some(mtm) = MainThreadMarker::new() else {
            return done(Err("not on the main thread".into()));
        };
        let info = NSPrintInfo::sharedPrintInfo();
        let paper = info.paperSize();
        // As wide as the paper's printable width, so the page is laid out as it prints.
        let width = paper.width - info.leftMargin() - info.rightMargin();
        let frame = NSRect::new(NSPoint::new(-20000., -20000.), NSSize::new(width, paper.height));
        let id = next_id();
        // SAFETY: a borderless window, never ordered in: WebKit lays a page
        // out only in a window.
        let window = unsafe {
            let window = NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                frame,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            );
            window.setReleasedWhenClosed(false);
            window
        };
        let config = unsafe { WKWebViewConfiguration::new(mtm) };
        let local = NSRect::new(NSPoint::new(0., 0.), frame.size);
        // SAFETY: WebKit's designated initialiser, on the main thread.
        let web = unsafe {
            WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), local, &config)
        };
        window.setContentView(Some(&web));
        let delegate: Retained<Loaded> = {
            let this = Loaded::alloc(mtm).set_ivars(LoadedIvars { id });
            unsafe { msg_send![super(this), init] }
        };
        unsafe {
            web.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            web.loadHTMLString_baseURL(&NSString::from_str(html), None);
        }
        PRINTING.with(|p| {
            p.borrow_mut().push(Printing {
                window,
                web,
                _delegate: delegate,
                op: None,
                job,
                title: title.to_string(),
                done,
            })
        });
    }

    /// The page `id` is laid out (or failed to be): its job runs, as a sheet
    /// of the window in front for a print, of the page's own window (never
    /// shown) for a PDF; it goes when the job is over (`finish`).
    fn print(id: usize, loaded: Result<(), String>) {
        if let Err(e) = loaded {
            return finish(id, Err(e));
        }
        let Some((web, window, delegate, to_pdf, title)) = PRINTING.with(|p| {
            let p = p.borrow();
            let x = p.iter().find(|x| x._delegate.ivars().id == id)?;
            let to_pdf = matches!(x.job, Job::Pdf(_));
            let info = NSPrintInfo::sharedPrintInfo();
            // SAFETY: a copy of the shared print info, changed for this job only.
            let info: Retained<NSPrintInfo> = unsafe { msg_send![&*info, copy] };
            info.setHorizontalPagination(NSPrintingPaginationMode::Fit);
            info.setHorizontallyCentered(false);
            info.setVerticallyCentered(false);
            if let Job::Pdf(path) = &x.job {
                let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
                // SAFETY: the job's attributes, keyed by AppKit's own keys.
                unsafe {
                    info.setJobDisposition(NSPrintSaveJob);
                    let dict = info.dictionary();
                    let _: () = msg_send![&*dict, setObject: &*url, forKey: NSPrintJobSavingURL];
                }
            }
            Some((
                x.web.clone(),
                x.window.clone(),
                x._delegate.clone(),
                to_pdf,
                (info, x.title.clone()),
            ))
        }) else {
            return;
        };
        let (info, title) = title;
        // SAFETY: WebKit's print operation for its own page.
        let op = unsafe { web.printOperationWithPrintInfo(&info) };
        op.setJobTitle(Some(&NSString::from_str(&title)));
        op.setShowsPrintPanel(!to_pdf);
        op.setShowsProgressPanel(false);
        // Without a frame WebKit's print view prints blank pages.
        if let Some(view) = op.view() {
            view.setFrame(web.frame());
        }
        let mtm = MainThreadMarker::new().expect("main thread");
        let front = NSApplication::sharedApplication(mtm).keyWindow();
        let sheet_of = match (&front, to_pdf) {
            (Some(front), false) => front.clone(),
            _ => window,
        };
        PRINTING.with(|p| {
            if let Some(x) = p.borrow_mut().iter_mut().find(|x| x._delegate.ivars().id == id) {
                x.op = Some(op.clone());
            }
        });
        // SAFETY: the delegate answers the selector, and lives in PRINTING
        // until it is called.
        unsafe {
            op.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
                &sheet_of,
                Some(&delegate),
                Some(objc2::sel!(printOperationDidRun:success:contextInfo:)),
                std::ptr::null_mut(),
            );
        }
    }

    /// The job of page `id` is over: its window closes, and `done` is told.
    fn finish(id: usize, result: Result<(), String>) {
        let Some(printing) = PRINTING.with(|p| {
            let mut p = p.borrow_mut();
            let i = p.iter().position(|x| x._delegate.ivars().id == id)?;
            Some(p.remove(i))
        }) else {
            return;
        };
        printing.window.close();
        let result = match (&printing.job, result) {
            (Job::Pdf(path), Ok(())) if !path.exists() => {
                Err("the PDF could not be written".to_string())
            }
            (_, r) => r,
        };
        (printing.done)(result);
    }
}
