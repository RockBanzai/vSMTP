import "/etc/vsmtp/plugins/libvsmtp_clamav_plugin" as clamav;

const bridge = clamav::connect(#{
    url: "tcp://clamav:3310",
});

fn on_post_queue(ctx) {
    for i in ctx.recipients {
        if i.domain is "example.com" {
            // set a routing maildir for my domain's recipients
            ctx.set_routing_path(i, "maildir");
        } else {
            // nothing to do here, default value is `basic`, equivalent to:
            // ctx.set_routing_path(i, "basic");
        }
    }

    if global::bridge.scan(ctx) {
        status::quarantine("virus")
    } else {
        status::success()
    }
}
