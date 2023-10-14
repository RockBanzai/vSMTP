#!/usr/bin/env bash

set -xe

if [ "$#" -ne 1 ]
then
  echo "Error: No domain name argument provided"
  echo "Usage: Provide a domain name as an argument"
  exit 1
fi

DOMAIN=$1

rm -rf $DOMAIN
mkdir -p $DOMAIN
cd $DOMAIN
mkdir -p rsa/ ecdsa/ eddsa/

openssl req -nodes \
  -x509 \
  -days 3650 \
  -newkey rsa:4096 \
  -keyout rsa/ca.key \
  -out rsa/ca.cert \
  -sha256 \
  -batch \
  -subj "/CN=ponytown RSA CA"

openssl req -nodes \
  -newkey rsa:3072 \
  -keyout rsa/inter.key \
  -out rsa/inter.req \
  -sha256 \
  -batch \
  -subj "/CN=ponytown RSA level 2 intermediate"

openssl req -nodes \
  -newkey rsa:2048 \
  -keyout rsa/end.key \
  -out rsa/end.req \
  -sha256 \
  -batch \
  -subj "/CN=$DOMAIN"

openssl rsa \
  -in rsa/end.key \
  -out rsa/end.rsa

openssl req -nodes \
  -newkey rsa:2048 \
  -keyout rsa/client.key \
  -out rsa/client.req \
  -sha256 \
  -batch \
  -subj "/CN=ponytown client"

openssl rsa \
  -in rsa/client.key \
  -out rsa/client.rsa

# ecdsa
openssl ecparam -name prime256v1 -out ecdsa/nistp256.pem
openssl ecparam -name secp384r1 -out ecdsa/nistp384.pem

openssl req -nodes \
  -x509 \
  -newkey ec:ecdsa/nistp384.pem \
  -keyout ecdsa/ca.key \
  -out ecdsa/ca.cert \
  -sha256 \
  -batch \
  -days 3650 \
  -subj "/CN=ponytown ECDSA CA"

openssl req -nodes \
  -newkey ec:ecdsa/nistp256.pem \
  -keyout ecdsa/inter.key \
  -out ecdsa/inter.req \
  -sha256 \
  -batch \
  -days 3000 \
  -subj "/CN=ponytown ECDSA level 2 intermediate"

openssl req -nodes \
  -newkey ec:ecdsa/nistp256.pem \
  -keyout ecdsa/end.key \
  -out ecdsa/end.req \
  -sha256 \
  -batch \
  -days 2000 \
  -subj "/CN=$DOMAIN"

openssl req -nodes \
  -newkey ec:ecdsa/nistp384.pem \
  -keyout ecdsa/client.key \
  -out ecdsa/client.req \
  -sha256 \
  -batch \
  -days 2000 \
  -subj "/CN=ponytown client"

# eddsa

openssl genpkey -algorithm Ed25519 -out eddsa/ca.key

openssl req -nodes \
  -x509 \
  -key eddsa/ca.key \
  -out eddsa/ca.cert \
  -sha256 \
  -batch \
  -days 3650 \
  -subj "/CN=ponytown EdDSA CA"

openssl genpkey -algorithm Ed25519 -out eddsa/inter.key

openssl req -nodes \
  -new \
  -key eddsa/inter.key \
  -out eddsa/inter.req \
  -sha256 \
  -batch \
  -subj "/CN=ponytown EdDSA level 2 intermediate"

openssl genpkey -algorithm Ed25519 -out eddsa/end.key

openssl req -nodes \
  -new \
  -key eddsa/end.key \
  -out eddsa/end.req \
  -sha256 \
  -batch \
  -subj "/CN=$DOMAIN"

openssl genpkey -algorithm Ed25519 -out eddsa/client.key

openssl req -nodes \
  -new \
  -key eddsa/client.key \
  -out eddsa/client.req \
  -sha256 \
  -batch \
  -subj "/CN=ponytown client"

cat > crl-openssl.cnf <<EOF
[ ca ]
default_ca = CA_default

[ CA_default ]
database        = ./index.txt
crlnumber       = ./crlnumber
default_md      = default
crl_extensions  = crl_ext

[ crl_ext ]
authorityKeyIdentifier=keyid:always
EOF

cat > openssl.cnf <<EOF
[ v3_end ]
basicConstraints = critical,CA:false
keyUsage = nonRepudiation, digitalSignature
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer:always
subjectAltName = @alt_names

[ v3_client ]
basicConstraints = critical,CA:false
keyUsage = nonRepudiation, digitalSignature
extendedKeyUsage = critical, clientAuth
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer:always

[ v3_inter ]
subjectKeyIdentifier = hash
extendedKeyUsage = critical, serverAuth, clientAuth
basicConstraints = CA:true
keyUsage = cRLSign, keyCertSign, digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment, keyAgreement, keyCertSign, cRLSign

[ alt_names ]
DNS.1 = $DOMAIN
IP.1 = 198.51.100.1
DNS.2 = second.$DOMAIN
IP.2 = 2001:db8::1
DNS.3 = localhost
EOF

for kt in rsa ecdsa eddsa ; do
  openssl x509 -req \
    -in $kt/inter.req \
    -out $kt/inter.cert \
    -CA $kt/ca.cert \
    -CAkey $kt/ca.key \
    -sha256 \
    -days 3650 \
    -set_serial 123 \
    -extensions v3_inter -extfile openssl.cnf

  openssl x509 -req \
    -in $kt/end.req \
    -out $kt/end.cert \
    -CA $kt/inter.cert \
    -CAkey $kt/inter.key \
    -sha256 \
    -days 2000 \
    -set_serial 456 \
    -extensions v3_end -extfile openssl.cnf

  openssl x509 -req \
    -in $kt/client.req \
    -out $kt/client.cert \
    -CA $kt/inter.cert \
    -CAkey $kt/inter.key \
    -sha256 \
    -days 2000 \
    -set_serial 789 \
    -extensions v3_client -extfile openssl.cnf

  echo -n '' > index.txt
  echo '1000' > crlnumber

  openssl ca \
    -config ./crl-openssl.cnf \
    -keyfile $kt/inter.key \
    -cert $kt/inter.cert \
    -gencrl \
    -crldays 7 \
    -revoke $kt/client.cert \
    -crl_reason keyCompromise \
    -out $kt/client.revoked.crl.pem

  openssl ca \
    -config ./crl-openssl.cnf \
    -keyfile $kt/inter.key \
    -cert $kt/inter.cert \
    -gencrl \
    -crldays 7 \
    -out $kt/client.revoked.crl.pem

  cat $kt/inter.cert $kt/ca.cert > $kt/end.chain
  cat $kt/end.cert $kt/inter.cert $kt/ca.cert > $kt/end.fullchain

  cat $kt/inter.cert $kt/ca.cert > $kt/client.chain
  cat $kt/client.cert $kt/inter.cert $kt/ca.cert > $kt/client.fullchain

  openssl asn1parse -in $kt/ca.cert -out $kt/ca.der > /dev/null
done

# Tidy up openssl CA state.
rm openssl.cnf crl-openssl.cnf
rm index.txt* || true
rm crlnumber* || true
