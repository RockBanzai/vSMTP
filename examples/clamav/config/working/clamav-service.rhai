import "/usr/lib/vsmtp/libvsmtp_plugin_clamav.so" as clamav;

export const antivirus = clamav::connect(#{
    url: "unix://clamav:3310",
});
