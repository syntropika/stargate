# Stargate identity

Stargate's Open Portal mark combines two angular gate sides with a four-point
star. It shares Syntropika's bronze palette, geometric precision and technical
typography, while using an independent product symbol.

- `mark.svg`: bronze mark on a transparent background.
- `mark-monochrome.svg`: single-color mark using `currentColor`.
- `logo-dark.svg`: outlined wordmark and mark on a near-black background.
- `logo-light.svg`: outlined wordmark and mark on a cream background.

Preserve the three-part geometry, proportions and clear internal spacing. Do not
add gradients, glow, texture or shadows. Use at least one quarter of the mark's
width as surrounding clear space. Prefer the mark alone for small icons and
favicons; keep it at least 24 pixels wide.

The palette is warm charcoal `#13120e`, ivory `#f4f0e8` and bronze `#c4a882`. Use
`#aaa79c` for readable secondary text on dark surfaces. The account panel and
public website use EB Garamond for editorial headings, Geist for interface text,
and JetBrains Mono for code and credential values. The public hero preserves the
mark geometry as animated ASCII characters in bronze and ivory, with a static
alternative for reduced motion.

The outlined wordmarks use JetBrains Mono Light. Its SIL Open Font License is
retained in `OFL-JetBrains-Mono.txt`. Generated concept images and operational
design records are not shipped as repository assets. The embedded account logo
in `crates/ui/assets/stargate.svg` mirrors `mark.svg`; update both together.
