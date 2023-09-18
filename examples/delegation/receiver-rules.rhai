fn on_pre_queue(ctx) {
    if ctx.server_port == 25 {
        for i in ctx.recipients {
            ctx.set_routing_path(i, "forward.av");
        }
        status::next()
    } else {
        if ctx.has_header("X-Virus-Infected") {
            status::quarantine("virus")
        } else {
            status::quarantine("done")
        }
    }
}
