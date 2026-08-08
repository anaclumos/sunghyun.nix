ObjC.import("Foundation");
ObjC.bindFunction("dlopen", ["pointer", ["string", "int"]]);
$.dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", 1);
ObjC.bindFunction("CGSGetSymbolicHotKeyValue", [
  "int",
  ["int", "unsigned short *", "unsigned short *", "unsigned int *"],
]);
ObjC.bindFunction("CGSIsSymbolicHotKeyEnabled", ["bool", ["int"]]);
ObjC.bindFunction("CGSSetSymbolicHotKeyEnabled", ["int", ["int", "bool"]]);

var MODIFIER_MASK = 0x1f0000;
var MAX_HOT_KEY = 512;
var RESERVED = @reservedChords@;

function claimants() {
  var found = [];
  for (var id = 0; id < MAX_HOT_KEY; id++) {
    var keyEquivalent = Ref();
    var virtualKey = Ref();
    var modifiers = Ref();
    if ($.CGSGetSymbolicHotKeyValue(id, keyEquivalent, virtualKey, modifiers) !== 0) {
      continue;
    }
    var masked = modifiers[0] & MODIFIER_MASK;
    for (var i = 0; i < RESERVED.length; i++) {
      if (RESERVED[i].virtualKey === virtualKey[0] && RESERVED[i].modifiers === masked) {
        found.push({
          id: id,
          reservedFor: RESERVED[i].reservedFor,
          keyEquivalent: keyEquivalent[0],
          virtualKey: virtualKey[0],
          modifiers: modifiers[0],
          enabled: $.CGSIsSymbolicHotKeyEnabled(id),
        });
      }
    }
  }
  return found;
}

function describe(c) {
  return (
    "symbolic hot key " +
    c.id +
    " claims " +
    c.reservedFor +
    " (key " +
    c.virtualKey +
    ", modifiers " +
    c.modifiers +
    ")"
  );
}

function run(argv) {
  var mode = argv.length > 0 ? argv[0] : "status";
  var found = claimants();
  if (mode === "status") {
    if (found.length === 0) {
      return "no system shortcut claims a reserved chord";
    }
    return found
      .map(function (c) {
        return describe(c) + " enabled=" + c.enabled;
      })
      .join("\n");
  }
  if (mode === "scan") {
    // Machine-readable for the persist half: id keyEquivalent virtualKey modifiers enabled
    return found
      .map(function (c) {
        return [c.id, c.keyEquivalent, c.virtualKey, c.modifiers, c.enabled].join(" ");
      })
      .join("\n");
  }
  if (mode === "enabled") {
    return "enabled=" + $.CGSIsSymbolicHotKeyEnabled(parseInt(argv[1], 10));
  }
  if (mode !== "disable") {
    throw new Error("unknown hotkeys mode: " + mode);
  }
  var lines = [];
  found.forEach(function (c) {
    if (!c.enabled) {
      return;
    }
    var status = $.CGSSetSymbolicHotKeyEnabled(c.id, false);
    if (status !== 0) {
      throw new Error("CGSSetSymbolicHotKeyEnabled(" + c.id + ") failed: " + status);
    }
    if ($.CGSIsSymbolicHotKeyEnabled(c.id)) {
      throw new Error("symbolic hot key " + c.id + " is still enabled after disabling it");
    }
    lines.push("disabled " + describe(c));
  });
  return lines.length === 0 ? "no enabled claimant to disable" : lines.join("\n");
}
