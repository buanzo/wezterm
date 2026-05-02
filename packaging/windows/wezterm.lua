local wezterm = require("wezterm")

local config = wezterm.config_builder()
config.font = wezterm.font_with_fallback({
  "Cascadia Mono",
  "Consolas",
  "Courier New",
})
config.font_size = 11.0

local existing_wslenv = os.getenv("WSLENV") or ""
local owt_wslenv = table.concat({
  "OWT_NATIVE_WINDOW_ONLY/u",
  "OWT_CONTROL_PLANE/u",
  "OWT_NATIVE_ENDPOINT/u",
  "OWT_NATIVE_TOKEN/u",
  "OWT_NATIVE_PROTOCOL/u",
  "OWT_NATIVE_PID/u",
  "OWT_CONFIG_SOURCE/u",
  "OWT_TERMINAL_FORK/u",
  "OWT_SOURCE_CWD/u",
}, ":")

if existing_wslenv ~= "" then
  owt_wslenv = existing_wslenv .. ":" .. owt_wslenv
end

config.default_prog = {
  "wsl.exe",
  "-d",
  "Ubuntu",
  "--cd",
  "/home/buanzo/git/tools",
  "--exec",
  "/bin/bash",
  "-l",
}

local native_endpoint = os.getenv("OWT_NATIVE_ENDPOINT") or ""
local native_token = os.getenv("OWT_NATIVE_TOKEN") or ""
local native_protocol = os.getenv("OWT_NATIVE_PROTOCOL") or ""
local native_pid = os.getenv("OWT_NATIVE_PID") or ""
local control_plane = "native-window-only"
local native_window_only = "1"

if native_endpoint ~= "" and native_token ~= "" then
  control_plane = "native-endpoint-status"
  native_window_only = "0"
end

config.set_environment_variables = {
  OWT_NATIVE_WINDOW_ONLY = native_window_only,
  OWT_CONTROL_PLANE = control_plane,
  OWT_NATIVE_ENDPOINT = native_endpoint,
  OWT_NATIVE_TOKEN = native_token,
  OWT_NATIVE_PROTOCOL = native_protocol,
  OWT_NATIVE_PID = native_pid,
  OWT_CONFIG_SOURCE = "owt-terminal/packaging/windows/wezterm.lua",
  OWT_TERMINAL_FORK = "1",
  OWT_SOURCE_CWD = "/home/buanzo/git/tools",
  WSLENV = owt_wslenv,
}

config.window_close_confirmation = "NeverPrompt"

return config
