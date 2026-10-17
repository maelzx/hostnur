<?php

namespace App\Console\Commands;

use Illuminate\Console\Command;
use Illuminate\Support\Facades\Process;

/**
 * Configure nginx + Let's Encrypt TLS for the panel's public hostname.
 *
 * Idempotent and re-runnable: safe for first install, a domain change, or
 * recovering from a failed issuance. Must be run as root (it writes the nginx
 * site, runs certbot and reloads nginx). The installer and the Debian package
 * post-install call this same command, so there is one implementation.
 */
class SetupPanelTls extends Command
{
    protected $signature = 'hostnur:panel:setup-tls
        {--domain= : Public hostname for the panel (e.g. panel.example.com)}
        {--email= : ACME contact email}
        {--force : Request a certificate even if one already exists}';

    protected $description = "Configure nginx + Let's Encrypt TLS for the panel domain (idempotent).";

    public function handle(): int
    {
        if (function_exists('posix_getuid') && posix_getuid() !== 0) {
            $this->error('hostnur:panel:setup-tls must be run as root.');

            return self::FAILURE;
        }

        $domain = trim((string) $this->option('domain'));
        $email = trim((string) $this->option('email'));

        if ($domain === '' || $email === '') {
            $this->error('Both --domain and --email are required.');

            return self::FAILURE;
        }

        if (preg_match('/^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$/', $domain) !== 1) {
            $this->error('The domain is not a valid lower-case hostname.');

            return self::FAILURE;
        }

        if (filter_var($email, FILTER_VALIDATE_EMAIL) === false) {
            $this->error('The email is not valid.');

            return self::FAILURE;
        }

        $root = rtrim((string) config('hostnur.panel.root', '/opt/hostnur-panel'), '/');
        $socket = (string) config('hostnur.panel.php_fpm_socket', '/run/php/hostnur-panel.sock');
        $site = (string) config('hostnur.panel.nginx_site', '/etc/nginx/sites-available/hostnur-panel');
        $webroot = (string) config('hostnur.panel.acme_webroot', '/var/www/hostnur-acme');

        if (! is_file($root.'/artisan')) {
            $this->error("The panel was not found at {$root}.");

            return self::FAILURE;
        }

        if (! $this->commandExists('certbot')) {
            $this->error('certbot is not installed (apt-get install certbot).');

            return self::FAILURE;
        }

        Process::run(['mkdir', '-p', $webroot.'/.well-known/acme-challenge']);

        // 1. Serve ACME challenges over HTTP first.
        $this->writeSite($site, $this->httpOnlySite($domain, $webroot));
        $this->enableSite($site);

        if (! $this->nginxReload()) {
            return self::FAILURE;
        }

        // 2. Obtain a certificate (skip when one already exists, unless forced).
        $certPath = "/etc/letsencrypt/live/{$domain}/fullchain.pem";

        if ($this->option('force') || ! is_file($certPath)) {
            $this->info("Requesting a Let's Encrypt certificate for {$domain}...");
            $result = Process::timeout(180)->run([
                'certbot', 'certonly', '--webroot', '-w', $webroot,
                '-d', $domain, '--agree-tos', '--no-eff-email',
                '-m', $email, '--non-interactive',
                // Reload nginx after future renewals (saved by certbot).
                '--deploy-hook', 'systemctl reload nginx',
            ]);

            if (! $result->successful()) {
                $this->error('Certificate issuance failed: '.trim($result->errorOutput() ?: $result->output()));

                // Keep the panel reachable over HTTP (avoid a 301-only site).
                $this->writeSite($site, $this->httpServeSite($domain, $root.'/public', $socket));
                $this->enableSite($site);
                $this->nginxReload();

                return self::FAILURE;
            }
        } else {
            $this->line('A certificate already exists; skipping issuance.');
        }

        // 3. Final HTTPS site with an HTTP redirect.
        $this->writeSite($site, $this->httpsSite($domain, $root.'/public', $socket, $webroot));
        $this->enableSite($site);

        if (! $this->nginxReload()) {
            return self::FAILURE;
        }

        // 4. Point the panel at the HTTPS URL.
        $this->setEnv($root.'/.env', 'APP_URL', "https://{$domain}");
        Process::path($root)->run(['php', 'artisan', 'config:clear']);

        $this->info("TLS enabled. The panel is served at https://{$domain}");

        return self::SUCCESS;
    }

    private function httpOnlySite(string $domain, string $webroot): string
    {
        return <<<NGINX
        server {
            listen 80 default_server;
            server_name {$domain} _;
            location ^~ /.well-known/acme-challenge/ { root {$webroot}; }
            location / { return 301 https://{$domain}\$request_uri; }
        }
        NGINX;
    }

    private function httpsSite(string $domain, string $public, string $socket, string $webroot): string
    {
        return <<<NGINX
        server {
            listen 80 default_server;
            server_name {$domain} _;
            location ^~ /.well-known/acme-challenge/ { root {$webroot}; }
            location / { return 301 https://{$domain}\$request_uri; }
        }
        server {
            listen 443 ssl;
            server_name {$domain};
            ssl_certificate /etc/letsencrypt/live/{$domain}/fullchain.pem;
            ssl_certificate_key /etc/letsencrypt/live/{$domain}/privkey.pem;
            root {$public};
            index index.php;
            location / { try_files \$uri \$uri/ /index.php?\$query_string; }
            location ~ \.php\$ {
                include snippets/fastcgi-php.conf;
                fastcgi_pass unix:{$socket};
            }
            location ~ /\.(?!well-known) { deny all; }
        }
        NGINX;
    }

    private function writeSite(string $site, string $content): void
    {
        $directory = dirname($site);

        if (! is_dir($directory)) {
            mkdir($directory, 0755, true);
        }

        file_put_contents($site, $content."\n");
    }

    private function enableSite(string $site): void
    {
        $enabled = '/etc/nginx/sites-enabled/'.basename($site);
        @unlink($enabled);
        symlink($site, $enabled);
    }

    private function nginxReload(): bool
    {
        $test = Process::run(['nginx', '-t']);

        if (! $test->successful()) {
            $this->error('nginx configuration test failed: '.trim($test->errorOutput()));

            return false;
        }

        Process::run(['systemctl', 'reload', 'nginx']);
        $this->line('nginx reloaded.');

        return true;
    }

    private function setEnv(string $envPath, string $key, string $value): void
    {
        $contents = is_file($envPath) ? (string) file_get_contents($envPath) : '';
        $line = $key.'='.$value;

        if (preg_match('/^'.preg_quote($key, '/').'=.*$/m', $contents) === 1) {
            $contents = preg_replace('/^'.preg_quote($key, '/').'=.*$/m', $line, $contents);
        } else {
            $contents = rtrim($contents, "\n")."\n".$line."\n";
        }

        file_put_contents($envPath, $contents);
    }

    private function commandExists(string $command): bool
    {
        return Process::run(['sh', '-c', 'command -v '.escapeshellarg($command)])->successful();
    }
}
