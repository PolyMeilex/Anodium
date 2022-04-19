Outputs.layout = [
    #{
        name: "HDMI-A-1",
        resolution: [1920, 1080],
        refresh: 60 * 1000
    },
    #{
        name: "eDP-1",
        resolution: [1920, 1080],
        refresh: 60 * 1000
    },
];

Keyboard.keybind(
    ["alt"], "return",
    || {
        Log.info("starting weston terminal");
        System.exec("weston-terminal");
    }
);

for key in 1..=9 {
    let key_name = key.to_string();
    Keyboard.keybind(
        ["alt"], key_name,
        || {
            Log.info("workspace: " + key_name);
        }
    );
}