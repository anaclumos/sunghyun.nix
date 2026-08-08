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

  environment.etc."claude-code/managed-settings.json".text = builtins.toJSON {
    env = {
      CLAUDE_AUTOCOMPACT_PCT_OVERRIDE = "50";
      CLAUDE_CODE_ATTRIBUTION_HEADER = "0";
      CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING = "1";
      CLAUDE_CODE_NO_FLICKER = "1";
      ENABLE_TOOL_SEARCH = "true";
    };
    includeCoAuthoredBy = false;
    extraKnownMarketplaces = {
      openai-codex = {
        source = {
          source = "github";
          repo = "openai/codex-plugin-cc";
        };
      };
    };
    ultracode = true;
    voice = {
      enabled = true;
      mode = "hold";
    };
    showThinkingSummaries = true;
    skipDangerousModePermissionPrompt = true;
    theme = "auto";
    autoCompactEnabled = true;
    switchModelsOnFlag = false;
    remoteControlAtStartup = false;
    agentPushNotifEnabled = true;
    voiceEnabled = true;
    model = "claude-fable-5[1m]";
  };
}
