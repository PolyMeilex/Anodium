print("config init!!!");

log::info("initalizing!!!");

anodize.keyboard.callbacks.register(|| {
    log::info("key called P");
}, "Super_L", ["p"]);

anodize.keyboard.callbacks.register(|| {
    log::info("key called H");
}, "Super_L", ["h"]);

anodize.keyboard.callbacks.register(|| {
    log::info("starting weston terminal");
    anodize.system.exec("weston-terminal");
}, "Control_L", ["t"]);


anodize.keyboard.callbacks.register(|| {
    this.focused_close();

}, "Control_L", ["y"]);

anodize.keyboard.callbacks.register(|| {
    this.focused_maximize();
}, "Control_L", ["w"]);


anodize.keyboard.callbacks.register_capture(|x| {
    log::info(x);
    this.switch_workspace(x);

}, "Control_L", ["s"], KeyCapture::Numbers());

anodize.keyboard.callbacks.register_capture(|x| {
    log::info(x);
}, "Control_L", ["l"], KeyCapture::Letters());


anodize.keyboard.callbacks.register(|| {
    let j = 0;
    _event_loop.add_timeout(|| {
        log::info("rhai timeout keyboard");
        j = j + 1;
        print(j);
        if j > 20 {
            return false
        }
        return true;
    }, 20);

}, "Control_L", ["k"]);

let i = 0;
anodize.keyboard.callbacks.register(|| {
    i = 1 + i;
    print(i);

}, "Control_L", ["i"]);

let x = 0;
_event_loop.add_timeout(|| {
    return false;
}, 500);

let c = 0;
anodize.keyboard.callbacks.register(|| {
    c = c + 1;
    print(c);
}, "Control_L", ["b"]);


anodize.keyboard.callbacks.register(|| {
    log::info("anodize");
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
