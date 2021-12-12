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
    anodize.log.info("anodize timeout");
    for output in anodize.outputs {
        output.info();
        output.set_wallpaper("bg.png")
    }
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


fn arrange_outputs(outputs) {
    let output_x = 0;

    outputs.reverse();

    outputs.map(|output| {
        output.x = output_x;
        output.y = 0;

        output_x += output.w;

        output
    })
}


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
