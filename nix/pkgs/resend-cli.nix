{
  lib,
  stdenvNoCC,
  fetchurl,
  makeBinaryWrapper,
  nodejs_24,
}:

stdenvNoCC.mkDerivation (finalAttrs: {
  pname = "resend-cli";
  version = "2.10.0";

  src = fetchurl {
    url = "https://registry.npmjs.org/resend-cli/-/resend-cli-${finalAttrs.version}.tgz";
    hash = "sha256-ohBxabPRJZfojijQSbmI0hWboFnZuAVLQWrQcQPC5lY=";
  };

  nativeBuildInputs = [ makeBinaryWrapper ];

  installPhase = ''
    runHook preInstall
    install -Dm644 dist/cli.cjs $out/lib/resend-cli/cli.cjs
    cp -r skills $out/lib/resend-cli/
    makeWrapper ${lib.getExe nodejs_24} $out/bin/resend \
      --add-flags $out/lib/resend-cli/cli.cjs
    runHook postInstall
  '';

  meta = {
    description = "Official CLI for Resend";
    homepage = "https://github.com/resend/resend-cli";
    license = lib.licenses.mit;
    mainProgram = "resend";
  };
})
