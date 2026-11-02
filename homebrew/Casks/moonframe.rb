# Template for moonframehq/homebrew-tap's Casks/moonframe.rb. The release's
# homebrew job fills in @VERSION@ and @SHA256@ (the published disk image's)
# with scripts/release/render-cask.sh and commits it to the tap.
cask "moonframe" do
  version "@VERSION@"
  sha256 "@SHA256@"

  url "https://github.com/moonframehq/moonframe/releases/download/v#{version}/Moonframe-#{version}-universal-apple-darwin.dmg"
  name "Moonframe"
  desc "Native desktop app whose whole window is written in Luau"
  homepage "https://moonframe.dev"

  # The app updates itself from its GitHub releases (Check for Updates), so
  # `brew upgrade` leaves it be unless asked with --greedy.
  auto_updates true
  depends_on macos: :ventura

  app "Moonframe.app"
  binary "#{appdir}/Moonframe.app/Contents/MacOS/moonframe"
end
