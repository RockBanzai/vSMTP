export const rsa = crypto::load_pem_rsa_pkcs8("/etc/vsmtp/dkim-private-key/rsa-private-key.pem");
export const ed  = crypto::load_pem_ed_pkcs8("/etc/vsmtp/dkim-private-key/ed25519-private-key.pem");
