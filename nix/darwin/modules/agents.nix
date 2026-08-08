{ pkgs, ... }:
let
  toml = pkgs.formats.toml { };
in
{
  environment.etc."codex/config.toml".source = toml.generate "codex-config.toml" {
    approval_policy = "never";
    approvals_reviewer = "user";
    model = "gpt-5.6-sol";
    model_reasoning_effort = "max";
    sandbox_mode = "danger-full-access";
    service_tier = "fast";
    agents.max_threads = 64;
  };
}
