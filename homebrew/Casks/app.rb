# Template for the casks of the apps Moonframe's releases ship, one each in
# moonframehq/homebrew-tap. scripts/release/render-app-casks.py fills it in
# for each app with a [release] table.
#
# It links no command: every bundle's executable is called `moonframe`, as is
# the starter's. `brew upgrade` updates it, as it has no updater of its own.
cask "@TOKEN@" do
  version "@VERSION@"
  sha256 "@SHA256@"

  url "https://github.com/moonframehq/moonframe/releases/download/apps-v#{version}/@ARCHIVE@-#{version}-universal-apple-darwin.zip"
  name "@TITLE@"
  desc "@DESCRIPTION@"
  homepage "https://github.com/moonframehq/moonframe/tree/main/examples/@NAME@"

  depends_on macos: :ventura

  app "@TITLE@.app"
end
