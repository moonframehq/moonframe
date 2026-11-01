# Template for penso/homebrew-tap's Casks/moonframe.rb. The release's
# homebrew job fills in @VERSION@ and @SHA256@ (the published disk image's)
# with scripts/release/render-cask.sh and commits it to the tap.
cask "moonframe" do
  version "@VERSION@"
  sha256 "@SHA256@"

  url "https://github.com/penso/moonframe/releases/download/v#{version}/Moonframe-#{version}-universal-apple-darwin.dmg"
  name "Moonframe"
  desc "Native desktop app whose whole window is written in Luau"
  homepage "https://moonframe.dev"

  depends_on macos: :ventura

  app "Moonframe.app"
  binary "#{appdir}/Moonframe.app/Contents/MacOS/moonframe"
end
