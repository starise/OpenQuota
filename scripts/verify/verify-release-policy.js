import assert from 'node:assert/strict';
import fs from 'node:fs';
import { parse } from 'yaml';

const read = (path) => fs.readFileSync(new URL(`../../${path}`, import.meta.url), 'utf8');
const release = parse(read('.github/workflows/release.yml'));
const { validate, 'publish-artifacts': build, 'publish-release': publish } = release.jobs;

assert.deepEqual(validate.permissions ?? release.permissions, { contents: 'read' });

const trustedTag = validate.steps.findIndex((step) => step.id === 'trusted_tag');
const credentials = validate.steps.findIndex((step) => step.env?.TAURI_SIGNING_PRIVATE_KEY);
assert.ok(
  trustedTag >= 0 && credentials > trustedTag,
  'Validate the tag before using signing credentials.',
);

const signingPolicy = validate.steps.find((step) => step.id === 'signing_policy');
for (const platform of ['WINDOWS', 'MACOS']) {
  const variable = `ENABLE_${platform}_NATIVE_SIGNING`;
  assert.equal(signingPolicy?.env?.[variable], `\${{ vars.${variable} || 'false' }}`);
}

const tagCommit = '${{ needs.validate.outputs.release_commit }}';
for (const jobId of ['prepare-release', 'publish-artifacts', 'publish-release']) {
  const checkout = release.jobs[jobId].steps.find((step) =>
    step.uses?.startsWith('actions/checkout@'),
  );
  assert.equal(checkout?.with?.ref, tagCommit, `${jobId} must check out the validated commit.`);
}

const smoke = build.steps.find((step) => step.uses?.endsWith('/.github/actions/platform-smoke'));
const smokeCheckout = build.steps.find(
  (step) =>
    step.uses?.startsWith('actions/checkout@') &&
    step.with?.path &&
    smoke?.uses === `./${step.with.path}/.github/actions/platform-smoke`,
);
assert.equal(
  smokeCheckout?.with?.ref,
  '${{ github.workflow_sha }}',
  'Use smoke tooling from the release workflow commit.',
);
assert.ok(
  !build['continue-on-error'] && smoke.if === undefined && !smoke['continue-on-error'],
  'Release smoke failures must block publication.',
);
assert.ok(
  publish.needs?.includes('publish-artifacts') && publish.if === undefined,
  'Publish only after the release build and smoke matrix succeeds.',
);

const { bundle } = JSON.parse(read('src-tauri/tauri.windows-signing.conf.json'));
assert.deepEqual(bundle.windows.signCommand, {
  cmd: 'cmd.exe',
  args: ['/d', '/s', '/c', 'openquota-sign-windows.cmd', '%1'],
});

console.log(
  'Release jobs enforce the validated commit, native signing defaults, and smoke-test gate.',
);
