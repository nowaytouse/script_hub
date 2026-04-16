/**
 * mosdns Forwarder for Surge
 *
 * Forwards all DNS queries to local mosdns instance listening on port 5335.
 * This bypasses Surge's limitation where dns-server cannot specify a port.
 *
 * @author nyamiiko
 * @version 2026.04.17
 */

$done({ server: "127.0.0.1", port: 5335 });
