#!/usr/bin/env python3
"""Tests of scripts/nu9ve_sign.py: identity choice, order and commands with a fake runner, and on macOS a real
ad-hoc signature of a throwaway bundle. Nothing touches the keychain or the network.

    python3 -I scripts/test-nu9ve-sign.py
"""

import plistlib
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import nu9ve_sign as sign  # noqa: E402

DEV_A = 'A' * 40
DEV_B = 'B' * 40
DIST = 'C' * 40
LISTING_ONE = '''  1) %s "Apple Development: Ana Pérez (TEAM123456)"
  2) %s "Developer ID Application: Ana Pérez (TEAM123456)"
     2 valid identities found
''' % (DEV_A, DIST)
LISTING_TWO = LISTING_ONE + '  3) %s "Apple Development: Ana Pérez (OTHER12345)"\n' % DEV_B
failures = []


def check(name, got, want):
    if got != want:
        failures.append('%s: got %r, want %r' % (name, got, want))


class Runner:
    def __init__(self, authority=''):
        self.calls = []
        self.authority = authority

    def __call__(self, command, check=False, capture_output=False, text=False):
        self.calls.append(command)
        return subprocess.CompletedProcess(command, 0, stdout='', stderr=self.authority)


def bundle(root):
    app = Path(root) / 'QA.app'
    for folder in ('Contents/MacOS', 'Contents/Helpers', 'Contents/Resources/runtime/lib/deep'):
        (app / folder).mkdir(parents=True)
    for target in ('Contents/MacOS/QA', 'Contents/Helpers/qa-engine', 'Contents/Resources/runtime/lib/a.dylib',
                   'Contents/Resources/runtime/lib/deep/b.dylib'):
        shutil.copyfile('/usr/bin/true', app / target)  # copy2 would copy the system's restricted flags
        (app / target).chmod(0o755)
    (app / 'Contents/Resources/runtime/lib/notes.txt').write_text('not Mach-O')
    with open(app / 'Contents/Info.plist', 'wb') as handle:
        plistlib.dump({'CFBundleExecutable': 'QA', 'CFBundleIdentifier': 'xyz.nu9ve.qa.sign',
                       'CFBundlePackageType': 'APPL', 'CFBundleName': 'QA'}, handle)
    return app


def identity_cases():
    check('ad hoc wins', sign.choose_identity('X', {'NU9VE_SIGN_IDENTITY': 'Y'}, ad_hoc=True, listing=LISTING_ONE)[0], '-')
    check('explicit before the variable', sign.choose_identity('X', {'NU9VE_SIGN_IDENTITY': 'Y'}, listing=LISTING_ONE)[0], 'X')
    check('the variable before the keychain', sign.choose_identity(None, {'NU9VE_SIGN_IDENTITY': 'Y'}, listing=LISTING_ONE)[0], 'Y')
    identity, note = sign.choose_identity(None, {}, listing=LISTING_ONE)
    check('the only Apple Development', identity, DEV_A)
    check('the note has no name', 'Ana' in note, False)
    check('distribution takes Developer ID', sign.choose_identity(None, {}, distribution=True, listing=LISTING_ONE)[0], DIST)
    identity, note = sign.choose_identity(None, {}, listing=LISTING_TWO)
    check('two are ambiguous', identity, '-')
    check('and the note says how many', '2 «Apple Development»' in note, True)
    check('none', sign.choose_identity(None, {}, listing='     0 valid identities found\n')[0], '-')
    check('a revoked line without a hash is skipped', sign.identities('  1) "Apple Development: x"\n', 'development'), [])


def command_cases():
    check('development build', sign.codesign_command('/a.app', DEV_A),
          ['/usr/bin/codesign', '--force', '--options', 'runtime', '--sign', DEV_A, '--timestamp=none', '/a.app'])
    check('distribution build', sign.codesign_command('/a.app', DIST, distribution=True, entitlements='e.plist'),
          ['/usr/bin/codesign', '--force', '--options', 'runtime', '--sign', DIST, '--timestamp',
           '--entitlements', 'e.plist', '/a.app'])
    try:
        sign.sign('/nowhere.app', '-', distribution=True, run=Runner())
        failures.append('distribution with ad hoc was accepted')
    except sign.SignError:
        pass


def order_and_run_cases(root):
    app = bundle(root)
    order = [str(p.relative_to(app.parent)) for p in
             sign.signing_order(app, inner=['Contents/Helpers/qa-engine'], nested=['Contents/Resources/runtime'])]
    check('inside out, deepest first, each once', order, [
        'QA.app/Contents/Resources/runtime/lib/deep/b.dylib', 'QA.app/Contents/Resources/runtime/lib/a.dylib',
        'QA.app/Contents/Helpers/qa-engine', 'QA.app'])
    runner = Runner()
    sign.sign(app, DEV_A, inner=['Contents/Helpers/qa-engine'], entitlements='e.plist', run=runner)
    check('entitlements only on the app', [('--entitlements' in c) for c in runner.calls], [False, True, False])
    check('verify last', runner.calls[-1], ['/usr/bin/codesign', '--verify', '--deep', '--strict', str(app)])
    try:
        sign.signing_order(app, inner=['Contents/Helpers/missing'])
        failures.append('a missing inner path was accepted')
    except sign.SignError:
        pass

    try:
        sign.notarize(app, 'qa', run=Runner(authority='Authority=Apple Development: x\n'))
        failures.append('notarized an app without Developer ID')
    except sign.SignError:
        pass
    runner = Runner(authority='Authority=Developer ID Application: x\n')
    sign.notarize(app, 'qa-profile', run=runner)
    steps = [c[1] if c[0] == '/usr/bin/xcrun' else Path(c[0]).name for c in runner.calls]
    check('notarize steps', steps, ['codesign', 'ditto', 'notarytool', 'stapler', 'spctl'])
    check('zip outside the bundle folder', Path(runner.calls[1][-1]).parent != app.parent, True)
    return app


def real_ad_hoc(app):
    """codesign itself, ad hoc: the order and flags are accepted and the bundle verifies."""
    sign.sign(app, '-', inner=['Contents/Helpers/qa-engine'], nested=['Contents/Resources/runtime'])
    shown = subprocess.run(['/usr/bin/codesign', '-dvv', str(app)], capture_output=True, text=True).stderr
    check('real: ad hoc', 'Signature=adhoc' in shown, True)
    check('real: hardened runtime', 'runtime' in shown, True)


def main():
    identity_cases()
    command_cases()
    with tempfile.TemporaryDirectory(prefix='nu9ve-sign-test-') as root:
        app = order_and_run_cases(root)
        if sys.platform == 'darwin':
            real_ad_hoc(app)
    if failures:
        print('\n'.join(failures), file=sys.stderr)
        return 1
    print('nu9ve_sign: all cases pass' + ('' if sys.platform == 'darwin' else ' (no real codesign off macOS)'))
    return 0


if __name__ == '__main__':
    sys.exit(main())
