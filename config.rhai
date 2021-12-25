anodize.log.info("initalizing!!!");

anodize.keyboard.callbacks.register(|| {
    anodize.log.info("key called P");
}, "Super_L", ["p"]);

anodize.keyboard.callbacks.register(|| {
    anodize.log.info("key called H");
}, "Super_L", ["h"]);

anodize.keyboard.callbacks.register(|| {
    anodize.log.info("starting weston terminal");
    anodize.system.exec("weston-terminal");
    anodize.system.exec("xterm");
}, "Control_L", ["t"]);


anodize.keyboard.callbacks.register(|| {
    if anodize.windows.focused != () {
        anodize.windows.focused.close()
    }
}, "Control_L", ["y"]);

anodize.keyboard.callbacks.register(|| {
    if anodize.windows.focused != () {
        anodize.windows.focused.maximize();
    }
}, "Control_L", ["w"]);

anodize.keyboard.callbacks.register(|| {
    if anodize.windows.focused != () {
        anodize.windows.focused.unmaximize()
    }
}, "Control_L", ["u"]);


anodize.keyboard.callbacks.register_capture(|x| {
    anodize.log.info(x);
    anodize.workspace.select(x);

}, "Control_L", ["s"], keyboard::numbers());

anodize.keyboard.callbacks.register_capture(|x| {
    anodize.log.info(x);
}, "Control_L", ["l"], keyboard::letters());


anodize.keyboard.callbacks.register(|| {
    let j = 0;
    anodize.system.add_timeout(|| {
        anodize.log.info("rhai timeout keyboard " + j.to_string());
        j = j + 1;
        if j > 20 {
            return false
        }
        return true;
    }, 20);

}, "Control_L", ["k"]);


anodize.system.add_timeout(|| {
    return false;
}, 500);

let b = 0;
anodize.keyboard.callbacks.register(|| {
    b = b + 1;
    anodize.log.info("rhai b pressed " + b.to_string());
}, "Control_L", ["b"]);


anodize.keyboard.callbacks.register(|| {
    anodize.log.info("anodize");
}, "Control_L", ["g"]);

anodize.keyboard.callbacks.register(|| {
    for output in anodize.outputs {
        output.set_wallpaper("bg.png");
    }
}, "Control_L", ["r"]);


let mode_index = 0;
anodize.keyboard.callbacks.register(|| {
    let output = anodize.outputs[0];
    let mode = output.modes[mode_index];
    if mode == () {
        mode_index = 0;
    } else {
        output.update_mode(mode);
        output.set_wallpaper("bg.png");
    }
    mode_index += 1;
}, "Control_L", ["equal"]);


let rearrange_timeout;
anodize.outputs.on_rearrange(|| {
    anodize.system.clear_timeout(rearrange_timeout);
    rearrange_timeout = anodize.system.add_timeout(|| {
        anodize.log.info("anodize rearrange");
        let output_x = 0;
        for output in anodize.outputs {
            output_x += output.w;
            anodize.log.info("searching for mode");
        }
        return false;
    }, 500);
});

anodize.outputs.on_new(|output| {
    anodize.log.info("anodize on_new");
    if output.name == "Virtual-1" {
        let mode = output.modes.filter(1440, 900, 60);
        if mode != () {
            output.update_mode(mode);
        }
    }

    output.set_wallpaper("bg.png");
});
