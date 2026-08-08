{
  lib,
  pkgs,
  data ? import ./data.nix,
  version ? "2.0.0",
}:
let
  hsCli = "/Applications/Hammerspoon.app/Contents/Frameworks/hs/hs";

  tileActions = lib.attrNames data.tiles ++ [ "fullscreen" ];

  appCases = lib.concatStrings (
    lib.mapAttrsToList (
      key: bundleId: "    ${key}) printf '%s' ${lib.escapeShellArg bundleId} ;;\n"
    ) data.apps
    ++ lib.mapAttrsToList (
      alias: target: "    ${alias}) printf '%s' ${lib.escapeShellArg data.apps.${target}} ;;\n"
    ) data.appAliases
  );

  tileAliasCases = lib.concatStrings (
    lib.mapAttrsToList (
      alias: target: "    ${alias}) name=\"${target}\" ;;\n"
    ) data.tileAliases
  );

  reservedChordsJson = builtins.toJSON (
    map (chord: {
      inherit (chord) reservedFor virtualKey modifiers;
    }) data.reservedChords
  );

  jxa = pkgs.runCommand "sunghyun-jxa" { } ''
    mkdir -p $out
    cp ${./jxa}/*.js $out/
    substituteInPlace $out/hotkeys.js \
      --replace-fail '@reservedChords@' ${lib.escapeShellArg reservedChordsJson}
  '';

  parts = [
    ./lib/common.sh
    ./lib/actions.sh
    ./lib/surfaces.sh
    ./lib/kanata.sh
    ./lib/verify.sh
    ./lib/post-switch.sh
    ./lib/main.sh
  ];

  body = builtins.replaceStrings
    [
      "@jxaDir@"
      "@hsCli@"
      "@imeAbc@"
      "@imeKorean@"
      "@tileActions@"
      "@appCases@"
      "@tileAliasCases@"
      "@browserBundleId@"
      "@terminalAliasBundleId@"
      "@terminalAliasTarget@"
      "@kanataLabel@"
      "@kanataMinVersion@"
      "@kanataDriverUrl@"
      "@version@"
    ]
    [
      "${jxa}"
      hsCli
      data.ime.abc
      data.ime.korean
      (lib.concatStringsSep " " tileActions)
      appCases
      tileAliasCases
      data.defaultBrowserBundleId
      data.terminalAlias.bundleId
      data.terminalAlias.target
      data.kanata.label
      data.kanata.minVersion
      data.kanata.driverPkgUrl
      version
    ]
    (lib.concatMapStringsSep "\n" builtins.readFile parts);
in
pkgs.writeShellApplication {
  name = "sunghyun";
  runtimeInputs = with pkgs; [
    coreutils
    findutils
    gnugrep
    gnused
    gawk
  ];
  text = ''
    ${body}
    main "$@"
  '';
  meta = {
    description = "Keyboard actions and residual OS-prompt surfaces for sunghyun.nix";
    mainProgram = "sunghyun";
  };
}
