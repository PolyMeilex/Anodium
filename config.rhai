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


let rearrange_timeout;
anodize.outputs.on_rearrange(|| {
    anodize.system.clear_timeout(rearrange_timeout);
    rearrange_timeout = anodize.system.add_timeout(|| {
        anodize.log.info("anodize on_rearrange");
        let output_x = 0;
        for output in anodize.outputs {
            output.x = output_x;

            anodize.log.info("output name is " + output.name);
            anodize.log.info("output width is " + output.w.to_string());
            anodize.log.info("output height is " + output.h.to_string());
            anodize.log.info("output x is " + output.x.to_string());
            anodize.log.info("output y is " + output.y.to_string());
            output.set_wallpaper("bg.png");

            output_x += output.w;

            for mode in output.modes {
                mode.info();
            }

            anodize.log.info("searching for mode");

            let mode = output.modes.filter(1280, 768, 60);
            if mode != () {
                mode.info();
                output.update_mode(mode);
            } else {
                anodize.log.error("mode not found");
            }
        }
        return false;
    }, 500);
});



fn configure_output(name, modes){
    let find_mode = |w, h, r|{
        for mode in modes {
            if mode.w == w && mode.h == h && mode.refresh == r{
                return mode;
            }
        }
    };

    switch name {
        "HDMI-A-1" => {
            find_mode.call(1920, 1080, 60)
        },
        "DP-1" => {
            find_mode.call(1920, 1080, 60)
        }
        _ => {}
    }
}
