<?php

namespace Tests\Unit;

use App\Support\GitRepositoryUrl;
use InvalidArgumentException;
use PHPUnit\Framework\Attributes\DataProvider;
use Tests\TestCase;

class GitRepositoryUrlTest extends TestCase
{
    #[DataProvider('validUrls')]
    public function test_valid_urls_are_normalized(string $input, string $expected): void
    {
        $this->assertSame($expected, GitRepositoryUrl::normalize($input));
    }

    /**
     * @return array<string, array{string, string}>
     */
    public static function validUrls(): array
    {
        return [
            'https' => ['https://github.com/Org/Repo.git', 'https://github.com/Org/Repo.git'],
            'https trailing slash' => ['https://gitlab.com/group/app/', 'https://gitlab.com/group/app/'],
            'ssh scheme' => ['ssh://git@github.com/org/repo.git', 'ssh://git@github.com/org/repo.git'],
            'scp like' => ['git@github.com:org/repo.git', 'git@github.com:org/repo.git'],
        ];
    }

    #[DataProvider('invalidUrls')]
    public function test_unsafe_urls_are_rejected(string $url): void
    {
        $this->expectException(InvalidArgumentException::class);

        GitRepositoryUrl::normalize($url);
    }

    /**
     * @return array<string, array{string}>
     */
    public static function invalidUrls(): array
    {
        return [
            'embedded credentials' => ['https://user:pass@github.com/org/repo.git'],
            'https token user' => ['https://token@github.com/org/repo.git'],
            'file scheme' => ['file:///etc/passwd'],
            'ext scheme' => ['ext::sh -c whoami'],
            'local path' => ['/var/repo'],
            'relative path' => ['../repo'],
            'query' => ['https://github.com/org/repo.git?token=abc'],
            'fragment' => ['https://github.com/org/repo.git#main'],
            'custom port' => ['https://github.com:8443/org/repo.git'],
            'ftp scheme' => ['ftp://github.com/org/repo.git'],
            'control char' => ["https://github.com/org/repo\ngit"],
            'space' => ['https://github.com/org/re po.git'],
            'option like path' => ['https://github.com/--upload-pack=x'],
            'parent traversal' => ['https://github.com/org/../repo.git'],
            'ssh non-git user' => ['ssh://root@github.com/org/repo.git'],
            'no host' => ['https:///org/repo.git'],
        ];
    }
}
