// `ui.video`: a frame placed in the view, through the tone transform
// (moonframe-frames' Tone::apply is the same sum for a probe).

#include <metal_stdlib>
using namespace metal;

struct Uniforms {
    float2 scale;   // drawable pixels per picture pixel
    float2 offset;  // the picture's top left, in drawable pixels
    float2 picture; // its size, in pixels
    float black;    // as the texture reads values, 0 to 1 for integers
    float span;     // white - black, likewise
    float gain;
    float inverse_gamma;
    int kind;       // 0 colour, 1 one value, 2 Bayer
    int red_x;      // where a Bayer pattern's red sits in its cell
    int red_y;
    int nearest;
};

struct Placed {
    float4 position [[position]];
};

vertex Placed video_vertex(uint id [[vertex_id]]) {
    float2 corner = float2(id & 1, id >> 1) * 2.0 - 1.0;
    return { float4(corner, 0.0, 1.0) };
}

static float site(texture2d<float> frame, int2 at, int2 last) {
    return frame.read(uint2(clamp(at, int2(0), last))).r;
}

// Bilinear demosaicing: quick and fine for a preview. An engine that wants
// better sends finished colour instead.
static float3 demosaic(texture2d<float> frame, int2 at, int2 red) {
    int2 last = int2(frame.get_width(), frame.get_height()) - 1;
    int2 cell = (at + red) & 1;
    float here = site(frame, at, last);
    float across = (site(frame, at + int2(-1, 0), last) + site(frame, at + int2(1, 0), last)) / 2;
    float along = (site(frame, at + int2(0, -1), last) + site(frame, at + int2(0, 1), last)) / 2;
    float diagonal = (site(frame, at + int2(-1, -1), last) + site(frame, at + int2(1, -1), last)
                    + site(frame, at + int2(-1, 1), last) + site(frame, at + int2(1, 1), last)) / 4;
    float cross = (across + along) / 2;
    if (cell.x == 0 && cell.y == 0) {
        return float3(here, cross, diagonal);
    }
    if (cell.x == 1 && cell.y == 1) {
        return float3(diagonal, cross, here);
    }
    return cell.y == 0 ? float3(across, here, along) : float3(along, here, across);
}

fragment float4 video_fragment(Placed in [[stage_in]],
                               constant Uniforms &u [[buffer(0)]],
                               texture2d<float> frame [[texture(0)]]) {
    float2 point = (in.position.xy - u.offset) / u.scale;
    if (any(point < 0.0) || any(point >= u.picture)) {
        return float4(0.0, 0.0, 0.0, 1.0);
    }
    constexpr sampler smooth(mag_filter::linear, min_filter::linear, coord::normalized);
    float3 color;
    if (u.kind == 2) {
        color = demosaic(frame, int2(point), int2(u.red_x, u.red_y));
    } else {
        float4 pixel = u.nearest != 0 ? frame.read(uint2(point))
                                      : frame.sample(smooth, point / u.picture);
        color = u.kind == 1 ? float3(pixel.r) : pixel.rgb;
    }
    color = max((color - u.black) / u.span * u.gain, 0.0);
    if (u.inverse_gamma != 1.0) {
        color = pow(color, u.inverse_gamma);
    }
    return float4(color, 1.0);
}

struct GuideVertex {
    float2 position;
    float4 color;
};

struct ColoredPlaced {
    float4 position [[position]];
    float4 color;
};

vertex ColoredPlaced guide_vertex(uint id [[vertex_id]],
                                  constant GuideVertex *vertices [[buffer(0)]]) {
    return { float4(vertices[id].position, 0.0, 1.0), vertices[id].color };
}

fragment float4 guide_fragment(ColoredPlaced in [[stage_in]]) {
    return in.color;
}
