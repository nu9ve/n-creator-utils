"""Signing and notarizing a macOS app of the suite, as nüün, nove and Stalker each did on their own.

Standard library only and Python 3.9, so an app runs its pinned copy with the system Python:

    python3 -I vendor/nu9ve-core/scripts/nu9ve_sign.py identity
    python3 -I vendor/nu9ve-core/scripts/nu9ve_sign.py sign dist/App.app --inner Contents/Helpers/app-engine \\
        --entitlements packaging/App.entitlements
    python3 -I vendor/nu9ve-core/scripts/nu9ve_sign.py notarize dist/App.app --profile app-notary

or imports it (sys.dont_write_bytecode = True first: a __pycache__ in vendor/ fails sync-core.sh --check).

The rules (docs/practices.md §2.1):

- Identity, in this order: --ad-hoc; the explicit one (the app's own variable, passed as --identity); NU9VE_SIGN_IDENTITY;
  the only valid «Apple Development» identity in the keychain (the only «Developer ID Application» one with
  --distribution); ad hoc. Two candidates are ambiguous and fall back to ad hoc, saying so. The log shows a SHA-1
  prefix, never a certificate name, because the name carries the person's.
- Signing goes inside out: every Mach-O under --nested (deepest first), then --inner in the order given, then the app.
  Always hardened runtime. A secure timestamp only with --distribution, since it needs the network.
- Then `codesign --verify --deep --strict`.
- Notarizing refuses an app not signed with Developer ID, zips it with ditto outside the bundle's folder, submits it
  with a notarytool keychain profile and waits, staples the ticket and asks Gatekeeper.
"""

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

AD_HOC = '-'
MACHO_MAGICS = {
    b'\xfe\xed\xfa\xce', b'\xfe\xed\xfa\xcf', b'\xce\xfa\xed\xfe', b'\xcf\xfa\xed\xfe',  # thin, both byte orders
    b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca',  # universal
}
KINDS = {'development': 'Apple Development', 'distribution': 'Developer ID Application'}


class SignError(Exception):
    pass


def identities(listing, kind):
    """SHA-1 of each valid identity of `kind` in `security find-identity -v -p codesigning` output."""
    label = '"' + KINDS[kind] + ':'
    found = []
    for line in listing.splitlines():
        parts = line.split()
        if label in line and len(parts) > 2 and len(parts[1]) == 40:
            found.append(parts[1])
    return found


def keychain_listing():
    try:
        return subprocess.run(['/usr/bin/security', 'find-identity', '-v', '-p', 'codesigning'],
                              capture_output=True, text=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return ''


def choose_identity(explicit=None, env=None, ad_hoc=False, distribution=False, listing=None):
    """(identity, note). `identity` is what codesign gets; `note` is safe for a build log."""
    env = os.environ if env is None else env
    if ad_hoc:
        return AD_HOC, 'ad hoc (asked)'
    if explicit:
        return explicit, 'the identity the app passed'
    if env.get('NU9VE_SIGN_IDENTITY'):
        return env['NU9VE_SIGN_IDENTITY'], 'NU9VE_SIGN_IDENTITY'
    kind = 'distribution' if distribution else 'development'
    found = identities(keychain_listing() if listing is None else listing, kind)
    if len(found) == 1:
        return found[0], '%s (%s…)' % (KINDS[kind], found[0][:8])
    if found:
        return AD_HOC, ('ad hoc: %d «%s» identities in the keychain; choose one with NU9VE_SIGN_IDENTITY'
                        % (len(found), KINDS[kind]))
    return AD_HOC, 'ad hoc: no «%s» identity in the keychain' % KINDS[kind]


def is_macho(path):
    try:
        with open(path, 'rb') as handle:
            return handle.read(4) in MACHO_MAGICS
    except OSError:
        return False


def signing_order(app, inner=(), nested=()):
    """Paths to sign, inside out, each once: Mach-O files under `nested` (deepest first), `inner`, then the app."""
    app = Path(app)
    order = []
    for folder in nested:
        root = app / folder
        if not root.is_dir():
            raise SignError('nested folder not found: %s' % folder)
        files = [p for p in root.rglob('*') if p.is_file() and not p.is_symlink() and is_macho(p)]
        order += sorted(files, key=lambda p: (-len(p.parts), str(p)))
    for item in inner:
        path = app / item
        if not path.exists():
            raise SignError('inner path not found: %s' % item)
        order.append(path)
    order.append(app)
    seen, unique = set(), []
    for path in order:
        if path not in seen:
            seen.add(path)
            unique.append(path)
    return unique


def codesign_command(path, identity, distribution=False, entitlements=None):
    command = ['/usr/bin/codesign', '--force', '--options', 'runtime', '--sign', identity,
               '--timestamp' if distribution and identity != AD_HOC else '--timestamp=none']
    if entitlements:
        command += ['--entitlements', str(entitlements)]
    return command + [str(path)]


def sign(app, identity, inner=(), nested=(), entitlements=None, distribution=False, run=subprocess.run):
    """Signs inside out and verifies. Entitlements go only on the app itself."""
    if distribution and identity == AD_HOC:
        raise SignError('a distribution build needs a Developer ID identity, not ad hoc')
    app = Path(app)
    for path in signing_order(app, inner, nested):
        run(codesign_command(path, identity, distribution, entitlements if path == app else None), check=True)
    run(['/usr/bin/codesign', '--verify', '--deep', '--strict', str(app)], check=True)


def signed_with_developer_id(app, run=subprocess.run):
    shown = run(['/usr/bin/codesign', '-dvv', str(app)], capture_output=True, text=True)
    return 'Authority=Developer ID Application:' in (shown.stderr or '') + (shown.stdout or '')


def notarize(app, profile, run=subprocess.run):
    """Submits, waits, staples and asks Gatekeeper. The zip lives in a temporary folder and is removed."""
    app = Path(app)
    if not signed_with_developer_id(app, run):
        raise SignError('notarization needs an app signed with Developer ID Application')
    with tempfile.TemporaryDirectory(prefix='nu9ve-notary-') as folder:
        archive = Path(folder) / (app.stem + '.zip')
        run(['/usr/bin/ditto', '-c', '-k', '--keepParent', str(app), str(archive)], check=True)
        run(['/usr/bin/xcrun', 'notarytool', 'submit', str(archive), '--keychain-profile', profile, '--wait'],
            check=True)
    run(['/usr/bin/xcrun', 'stapler', 'staple', str(app)], check=True)
    run(['/usr/sbin/spctl', '--assess', '--type', 'execute', '--verbose', str(app)], check=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description='Sign and notarize a macOS app of the suite.')
    commands = parser.add_subparsers(dest='command', required=True)

    def identity_options(command):
        command.add_argument('--identity', help="the app's own identity (name or SHA-1), before NU9VE_SIGN_IDENTITY")
        command.add_argument('--ad-hoc', action='store_true', help='sign ad hoc even if an identity exists')
        command.add_argument('--distribution', action='store_true',
                             help='Developer ID and a secure timestamp, for a build to notarize')

    show = commands.add_parser('identity', help='print the identity that sign would use')
    identity_options(show)
    signer = commands.add_parser('sign', help='sign an .app inside out and verify it')
    signer.add_argument('app')
    identity_options(signer)
    signer.add_argument('--inner', action='append', default=[], help='a path inside the app to sign before it')
    signer.add_argument('--nested', action='append', default=[],
                        help='a folder inside the app whose Mach-O files are all signed, deepest first')
    signer.add_argument('--entitlements', help='entitlements for the app itself')
    notary = commands.add_parser('notarize', help='notarize, staple and assess a Developer ID app')
    notary.add_argument('app')
    notary.add_argument('--profile', required=True, help='notarytool keychain profile (xcrun notarytool store-credentials)')
    args = parser.parse_args(argv)

    try:
        if args.command == 'notarize':
            notarize(args.app, args.profile)
            print('Notarized and stapled: ' + args.app)
            return 0
        identity, note = choose_identity(args.identity, ad_hoc=args.ad_hoc, distribution=args.distribution)
        if args.command == 'identity':
            print(identity if identity == AD_HOC or len(identity) == 40 else 'explicit')
            print('Signing: ' + note, file=sys.stderr)
            return 0
        print('Signing: ' + note, file=sys.stderr)
        if identity == AD_HOC:
            print('Warning: ad hoc changes with every build, so macOS asks for its permissions again.', file=sys.stderr)
        sign(args.app, identity, args.inner, args.nested, args.entitlements, args.distribution)
        print('Signed and verified: ' + args.app)
        return 0
    except (SignError, subprocess.CalledProcessError) as error:
        print('nu9ve_sign: %s' % error, file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
