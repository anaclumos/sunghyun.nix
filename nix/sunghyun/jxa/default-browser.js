ObjC.import("AppKit");
ObjC.bindFunction("LSCopyDefaultHandlerForURLScheme", ["id", ["id"]]);

// macOS has gated the default-browser change behind its own confirmation panel
// since 10.13 and has no configuration profile for it, so the supported call is
// NSWorkspace's and the panel it raises is the human surface. Only http is set:
// macOS derives https and HTML from it and rejects a direct https change.
function handler() {
  var id = $.LSCopyDefaultHandlerForURLScheme($("http"));
  return id.js ? ObjC.unwrap(id) : "";
}

function run(argv) {
  var mode = argv.length > 0 ? argv[0] : "status";
  if (mode === "status") {
    var current = handler();
    return "default_browser=" + (current === "" ? "unknown" : current);
  }
  if (mode !== "set" && mode !== "installed") {
    throw new Error("unknown default-browser mode: " + mode);
  }
  var bundleId = argv[1];
  if (!bundleId) {
    throw new Error("default-browser " + mode + " requires a bundle id");
  }
  var workspace = $.NSWorkspace.sharedWorkspace;
  var url = workspace.URLForApplicationWithBundleIdentifier($(bundleId));
  if (mode === "installed") {
    if (!url.js) {
      throw new Error(bundleId + " is not installed");
    }
    return "installed " + ObjC.unwrap(url.path);
  }
  if (!url.js) {
    throw new Error("no application bundle registered for " + bundleId);
  }
  workspace.setDefaultApplicationAtURLToOpenURLsWithSchemeCompletionHandler(url, $("http"), null);
  // CoreServicesUIAgent owns the panel and normally comes forward on its own;
  // this makes sure it is never left sitting behind another window.
  var agents = $.NSRunningApplication.runningApplicationsWithBundleIdentifier(
    $("com.apple.coreservices.uiagent")
  );
  for (var i = 0; i < ObjC.unwrap(agents.count); i++) {
    agents.objectAtIndex(i).activateWithOptions(1 << 1);
  }
  return "requested " + bundleId;
}
