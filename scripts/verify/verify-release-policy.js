import assert from 'node:assert/strict';
import fs from 'node:fs';
import { parse } from 'yaml';

const read = (path) => fs.readFileSync(new URL(`../../${path}`, import.meta.url), 'utf8');
const release = parse(read('.github/workflows/release.yml'));
const {
  validate,
  'publish-artifacts': build,
  'build-macos': macos,
  'publish-release': publish,
} = release.jobs;

assert.deepEqual(validate.permissions ?? release.permissions, { contents: 'read' });
for (const job of [validate, build, macos]) {
  assert.equal(job.environment, 'release-signing');
}

const trustedTag = validate.steps.findIndex((step) => step.id === 'trusted_tag');
const credentials = validate.steps.findIndex((step) => step.env?.TAURI_SIGNING_PRIVATE_KEY);
assert.ok(
  trustedTag >= 0 && credentials > trustedTag,
  'Validate the tag before using signing credentials.',
);

const windowsSigning = validate.steps.find((step) => step.id === 'signing_policy');
const macosSigning = macos.steps.find((step) => step.id === 'signing');
assert.equal(
  windowsSigning?.env?.ENABLE_WINDOWS_NATIVE_SIGNING,
  `\${{ vars.ENABLE_WINDOWS_NATIVE_SIGNING || 'false' }}`,
);
assert.equal(
  macosSigning?.env?.ENABLE_MACOS_NATIVE_SIGNING,
  `\${{ vars.ENABLE_MACOS_NATIVE_SIGNING || 'false' }}`,
);

const tagCommit = `\${{ needs.validate.outputs.release_commit }}`;
for (const jobId of ['prepare-release', 'publish-artifacts', 'build-macos', 'publish-release']) {
  const checkout = release.jobs[jobId].steps.find((step) =>
    step.uses?.startsWith('actions/checkout@'),
  );
  assert.equal(checkout?.with?.ref, tagCommit, `${jobId} must check out the validated commit.`);
}

for (const job of [build, macos]) {
  const smoke = job.steps.find((step) => step.uses?.endsWith('/.github/actions/platform-smoke'));
  const smokeCheckout = job.steps.find(
    (step) =>
      step.uses?.startsWith('actions/checkout@') &&
      step.with?.path &&
      smoke?.uses === `./${step.with.path}/.github/actions/platform-smoke`,
  );
  assert.equal(
    smokeCheckout?.with?.ref,
    `\${{ github.workflow_sha }}`,
    'Use smoke tooling from the release workflow commit.',
  );
  assert.ok(
    !job['continue-on-error'] && smoke.if === undefined && !smoke['continue-on-error'],
    'A failed release smoke test must block publication for that platform.',
  );
}

const macosSmoke = macos.steps.findIndex((step) =>
  step.uses?.endsWith('/.github/actions/platform-smoke'),
);
const macosUpload = macos.steps.findIndex((step) =>
  step.uses?.startsWith('actions/upload-artifact@'),
);
assert.ok(macosUpload > macosSmoke, 'Stage the macOS artifact only after its smoke test.');
assert.deepEqual(macos.permissions ?? release.permissions, { contents: 'read' });
assert.deepEqual(publish.needs?.toSorted(), [
  'build-macos',
  'prepare-release',
  'publish-artifacts',
  'validate',
]);
assert.equal(
  publish.if?.trim().replace(/\s+/g, ' '),
  "!cancelled() && needs.validate.result == 'success' && needs.prepare-release.result == 'success' && needs.publish-artifacts.result == 'success'",
  'Publication must require the Windows and Linux smoke matrix and tolerate a missing macOS build.',
);

const { bundle } = JSON.parse(read('src-tauri/tauri.windows-signing.conf.json'));
assert.deepEqual(bundle.windows.signCommand, {
  cmd: 'cmd.exe',
  args: ['/d', '/s', '/c', 'openquota-sign-windows.cmd', '%1'],
});

console.log(
  'Release jobs enforce the validated commit, native signing defaults, and smoke-test gate.',
);
