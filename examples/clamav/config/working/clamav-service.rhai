import "/usr/lib/vsmtp/libvsmtp_clamav_plugin.so" as clamav;

export const antivirus = clamav::connect(#{
    url: "unix://clamav:3310",
});