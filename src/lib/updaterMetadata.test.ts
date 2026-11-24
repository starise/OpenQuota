import { describe, expect, it } from 'vitest';
import {
  createMacosUpdaterMetadata,
  createUpdaterMetadata,
} from '../../.github/scripts/create-updater-json.mjs';

describe('release updater metadata', () => {
  const artifactNames = [
    'OpenQuota_0.2.0_x64-setup.exe',
    'OpenQuota_0.2.0_arm64-setup.exe',
    'OpenQuota_0.2.0_amd64.AppImage',
    'OpenQuota_0.2.0_arm64.AppImage',
    'OpenQuota_0.2.0_amd64.deb',
    'OpenQuota_0.2.0_arm64.deb',
    'OpenQuota_0.2.0_universal.app.tar.gz',
  ];
  const release = {
    body: 'Release notes',
    created_at: '2026-07-14T00:00:00Z',
    assets: [
      ...artifactNames.flatMap((name, index) => [
        { id: index * 2 + 1, name },
        { id: index * 2 + 2, name: `${name}.sig` },
      ]),
      { id: 99, name: 'OpenQuota_0.2.0_universal.dmg' },
    ],
  };
  const signatures = Object.fromEntries(
    artifactNames.map((name) => [`${name}.sig`, `signature:${name}`]),
  );

  it('creates deterministic updater entries for every supported architecture', () => {
    const update = createUpdaterMetadata(
      release,
      signatures,
      'starise/OpenQuota',
      'v0.2.0',
      '0.2.0',
    );

    expect(update.platforms['windows-x86_64'].url).toBe(
      'https://github.com/starise/OpenQuota/releases/download/v0.2.0/OpenQuota_0.2.0_x64-setup.exe',
    );
    expect(update.platforms['windows-aarch64'].url).toContain('_arm64-setup.exe');
    expect(update.platforms['linux-aarch64'].url).toContain('_arm64.AppImage');
    expect(update.platforms['darwin-aarch64'].url).toContain('_universal.app.tar.gz');
    expect(Object.keys(update.platforms)).toHaveLength(16);
    expect(Object.keys(update.platforms)).toEqual(
      expect.arrayContaining([
        'windows-x86_64',
        'windows-aarch64',
        'linux-x86_64',
        'linux-aarch64',
        'darwin-x86_64',
        'darwin-aarch64',
      ]),
    );
  });

  it('rejects a release whose ARM64 artifact is missing', () => {
    expect(() =>
      createUpdaterMetadata(
        {
          ...release,
          assets: release.assets.filter(
            (asset) => !asset.name.startsWith('OpenQuota_0.2.0_arm64-setup.exe'),
          ),
        },
        signatures,
        'starise/OpenQuota',
        'v0.2.0',
        '0.2.0',
      ),
    ).toThrow('Expected one Windows ARM64 NSIS installer, found 0');
  });

  it('creates a macOS-only manifest with the same release metadata', () => {
    const complete = createUpdaterMetadata(
      release,
      signatures,
      'starise/OpenQuota',
      'v0.2.0',
      '0.2.0',
    );
    const macos = createMacosUpdaterMetadata(complete);

    expect(macos.version).toBe(complete.version);
    expect(macos.pub_date).toBe(complete.pub_date);
    expect(macos.notes).toBe(complete.notes);
    expect(Object.keys(macos.platforms)).toEqual([
      'darwin-aarch64',
      'darwin-x86_64',
      'darwin-universal',
      'darwin-aarch64-app',
      'darwin-x86_64-app',
      'darwin-universal-app',
    ]);
    expect(macos.platforms['darwin-universal'].url).toBe(
      'https://github.com/starise/OpenQuota/releases/download/v0.2.0/OpenQuota_0.2.0_universal.app.tar.gz',
    );
  });

  it('creates Windows and Linux metadata when macOS is explicitly omitted', () => {
    const releaseWithoutMacos = {
      ...release,
      assets: release.assets.filter((asset) => !asset.name.includes('_universal.')),
    };
    const update = createUpdaterMetadata(
      releaseWithoutMacos,
      signatures,
      'starise/OpenQuota',
      'v0.2.0',
      '0.2.0',
      { includeMacos: false },
    );

    expect(Object.keys(update.platforms)).toEqual([
      'windows-x86_64',
      'windows-x86_64-nsis',
      'windows-aarch64',
      'windows-aarch64-nsis',
      'linux-x86_64',
      'linux-x86_64-appimage',
      'linux-aarch64',
      'linux-aarch64-appimage',
      'linux-x86_64-deb',
      'linux-aarch64-deb',
    ]);
    expect(update.version).toBe('0.2.0');
  });

  it('rejects current-version macOS artifacts when macOS is omitted', () => {
    const releaseWithMacDmgOnly = {
      ...release,
      assets: release.assets.filter((asset) => !asset.name.endsWith('.app.tar.gz')),
    };

    expect(() =>
      createUpdaterMetadata(
        releaseWithMacDmgOnly,
        signatures,
        'starise/OpenQuota',
        'v0.2.0',
        '0.2.0',
        { includeMacos: false },
      ),
    ).toThrow('macOS artifacts for release version 0.2.0 are not allowed in this mode');
  });

  it('rejects a current-version macOS signature without its archive', () => {
    const releaseWithMacSignatureOnly = {
      ...release,
      assets: release.assets.filter(
        (asset) => !asset.name.endsWith('.app.tar.gz') && !asset.name.endsWith('.universal.dmg'),
      ),
    };

    expect(() =>
      createUpdaterMetadata(
        releaseWithMacSignatureOnly,
        signatures,
        'starise/OpenQuota',
        'v0.2.0',
        '0.2.0',
        { includeMacos: false },
      ),
    ).toThrow('macOS artifacts for release version 0.2.0 are not allowed in this mode');
  });

  it.each([
    {
      label: 'the archive signature',
      assets: release.assets.filter(
        (asset) => asset.name !== 'OpenQuota_0.2.0_universal.app.tar.gz.sig',
      ),
      signatures,
      error: 'Missing updater signature asset: OpenQuota_0.2.0_universal.app.tar.gz.sig',
    },
    {
      label: 'the downloaded archive signature',
      assets: release.assets,
      signatures: Object.fromEntries(
        Object.entries(signatures).filter(
          ([name]) => name !== 'OpenQuota_0.2.0_universal.app.tar.gz.sig',
        ),
      ),
      error: 'Missing downloaded updater signature: OpenQuota_0.2.0_universal.app.tar.gz.sig',
    },
    {
      label: 'the DMG',
      assets: release.assets.filter((asset) => asset.name !== 'OpenQuota_0.2.0_universal.dmg'),
      signatures,
      error: 'Expected one macOS Universal DMG, found 0',
    },
  ])(
    'rejects a Mac release missing $label',
    ({ assets, signatures: availableSignatures, error }) => {
      expect(() =>
        createUpdaterMetadata(
          { ...release, assets },
          availableSignatures,
          'starise/OpenQuota',
          'v0.2.0',
          '0.2.0',
        ),
      ).toThrow(error);
    },
  );
});
