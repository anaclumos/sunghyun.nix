ObjC.import("Foundation");
ObjC.bindFunction("TISCreateInputSourceList", ["id", ["id", "bool"]]);
ObjC.bindFunction("TISSelectInputSource", ["int", ["id"]]);
ObjC.bindFunction("TISCopyCurrentKeyboardInputSource", ["id", []]);
ObjC.bindFunction("TISGetInputSourceProperty", ["id", ["id", "id"]]);

// kTISPropertyInputSourceID's CFString value; TIS matches dictionary keys by
// CFEqual, so an equal string works without linking Carbon's constants.
var ID_KEY = "TISPropertyInputSourceID";

function currentId() {
  var source = $.TISCopyCurrentKeyboardInputSource();
  return ObjC.unwrap($.TISGetInputSourceProperty(source, $(ID_KEY)));
}

function run(argv) {
  if (argv.length === 0 || argv[0] === "status") {
    return "input_source=" + currentId();
  }
  var wanted = argv[0];
  if (currentId() === wanted) {
    return "input_source=" + wanted;
  }
  var filter = $.NSDictionary.dictionaryWithObjectForKey($(wanted), $(ID_KEY));
  var list = $.TISCreateInputSourceList(filter, true);
  if (!list.js || ObjC.unwrap(list.count) === 0) {
    throw new Error("input source not installed: " + wanted);
  }
  var status = $.TISSelectInputSource(list.objectAtIndex(0));
  if (status !== 0) {
    // paramErr (-50) here means installed but not enabled in Keyboard settings.
    throw new Error("TISSelectInputSource failed with status " + status + " for " + wanted);
  }
  return "input_source=" + wanted;
}
