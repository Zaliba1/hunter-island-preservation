// This file is part of the FidelityFX SDK.
//
// Copyright (C) 2024 Advanced Micro Devices, Inc.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files(the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and /or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions :
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

// GLSL 1.20 adaptation of AMD FidelityFX Super Resolution 1.2 (SDK v1.1.4).
// The reference's texture gathers are replaced with explicit texel-center samples.

uniform sampler2D image;
uniform vec2 imageSize;
varying vec2 uv;

vec3 loadPixel(vec2 pixel) {
    vec2 center = clamp(pixel, vec2(0.0), imageSize - 1.0) + 0.5;
    return texture2D(image, center / imageSize).rgb;
}

float luma(vec3 color) {
    return dot(color, vec3(0.5, 1.0, 0.5));
}

#ifdef SHARPEN
uniform vec2 viewportOrigin;

void main() {
    vec2 pixel = floor(gl_FragCoord.xy - viewportOrigin);
    vec3 north = loadPixel(pixel + vec2(0.0, -1.0));
    vec3 west = loadPixel(pixel + vec2(-1.0, 0.0));
    vec3 center = loadPixel(pixel);
    vec3 east = loadPixel(pixel + vec2(1.0, 0.0));
    vec3 south = loadPixel(pixel + vec2(0.0, 1.0));
    vec3 low = min(min(north, west), min(east, south));
    vec3 high = max(max(north, west), max(east, south));
    vec3 hitLow = min(low, center) / max(4.0 * high, vec3(1e-6));
    vec3 hitHigh = -(1.0 - max(high, center)) / max(4.0 * (1.0 - low), vec3(1e-6));
    vec3 limit = max(-hitLow, hitHigh);
    // Two stops below maximum RCAS sharpening.
    float lobe = clamp(max(max(limit.r, limit.g), limit.b), -0.1875, 0.0) * 0.25;
    vec3 sharpened = (center + lobe * (north + west + east + south)) / (1.0 + 4.0 * lobe);
    gl_FragColor = vec4(sharpened, 1.0);
}
#else

void accumulateEdge(inout vec2 direction, inout float edge, float weight,
                    float north, float west, float center, float east, float south) {
    vec2 across = vec2(east - west, south - north);
    vec2 strongest = max(abs(vec2(east, south) - center), abs(center - vec2(west, north)));
    vec2 gradient = clamp(abs(across) / max(strongest, vec2(1e-6)), 0.0, 1.0);
    direction += weight * across;
    edge += weight * dot(gradient, gradient);
}

void accumulateTap(inout vec3 sum, inout float total, vec2 offset, vec2 direction,
                   vec2 stretch, float lobe, vec3 color) {
    vec2 rotated = vec2(dot(offset, direction), dot(offset, vec2(-direction.y, direction.x)));
    vec2 stretched = rotated * stretch;
    float distanceSquared = min(dot(stretched, stretched), 1.0 / lobe);
    float window = lobe * distanceSquared - 1.0;
    float base = 0.4 * distanceSquared - 1.0;
    float weight = (1.5625 * base * base - 0.5625) * window * window;
    sum += weight * color;
    total += weight;
}

void main() {
    // The existing texture transform rotates about zero and wraps into [0, 1).
    vec2 position = fract(uv) * imageSize - 0.5;
    vec2 origin = floor(position);
    vec2 subpixel = position - origin;
    vec3 b = loadPixel(origin + vec2( 0.0, -1.0));
    vec3 c = loadPixel(origin + vec2( 1.0, -1.0));
    vec3 e = loadPixel(origin + vec2(-1.0,  0.0));
    vec3 f = loadPixel(origin);
    vec3 g = loadPixel(origin + vec2( 1.0,  0.0));
    vec3 h = loadPixel(origin + vec2( 2.0,  0.0));
    vec3 i = loadPixel(origin + vec2(-1.0,  1.0));
    vec3 j = loadPixel(origin + vec2( 0.0,  1.0));
    vec3 k = loadPixel(origin + vec2( 1.0,  1.0));
    vec3 l = loadPixel(origin + vec2( 2.0,  1.0));
    vec3 n = loadPixel(origin + vec2( 0.0,  2.0));
    vec3 o = loadPixel(origin + vec2( 1.0,  2.0));

    vec2 direction = vec2(0.0);
    float edge = 0.0;
    float x = subpixel.x;
    float y = subpixel.y;
    accumulateEdge(direction, edge, (1.0-x)*(1.0-y), luma(b), luma(e), luma(f), luma(g), luma(j));
    accumulateEdge(direction, edge, x*(1.0-y),       luma(c), luma(f), luma(g), luma(h), luma(k));
    accumulateEdge(direction, edge, (1.0-x)*y,       luma(f), luma(i), luma(j), luma(k), luma(n));
    accumulateEdge(direction, edge, x*y,             luma(g), luma(j), luma(k), luma(l), luma(o));

    float magnitudeSquared = dot(direction, direction);
    direction = magnitudeSquared < (1.0 / 32768.0)
        ? vec2(1.0, 0.0) : direction * inversesqrt(magnitudeSquared);
    edge *= 0.5;
    edge *= edge;
    float diagonalStretch = 1.0 / max(abs(direction.x), abs(direction.y));
    vec2 stretch = vec2(mix(1.0, diagonalStretch, edge), 1.0 - 0.5 * edge);
    float lobe = mix(0.5, 0.21, edge);

    vec3 sum = vec3(0.0);
    float total = 0.0;
    accumulateTap(sum, total, vec2( 0.0, -1.0) - subpixel, direction, stretch, lobe, b);
    accumulateTap(sum, total, vec2( 1.0, -1.0) - subpixel, direction, stretch, lobe, c);
    accumulateTap(sum, total, vec2(-1.0,  0.0) - subpixel, direction, stretch, lobe, e);
    accumulateTap(sum, total,                    - subpixel, direction, stretch, lobe, f);
    accumulateTap(sum, total, vec2( 1.0,  0.0) - subpixel, direction, stretch, lobe, g);
    accumulateTap(sum, total, vec2( 2.0,  0.0) - subpixel, direction, stretch, lobe, h);
    accumulateTap(sum, total, vec2(-1.0,  1.0) - subpixel, direction, stretch, lobe, i);
    accumulateTap(sum, total, vec2( 0.0,  1.0) - subpixel, direction, stretch, lobe, j);
    accumulateTap(sum, total, vec2( 1.0,  1.0) - subpixel, direction, stretch, lobe, k);
    accumulateTap(sum, total, vec2( 2.0,  1.0) - subpixel, direction, stretch, lobe, l);
    accumulateTap(sum, total, vec2( 0.0,  2.0) - subpixel, direction, stretch, lobe, n);
    accumulateTap(sum, total, vec2( 1.0,  2.0) - subpixel, direction, stretch, lobe, o);

    vec3 low = min(min(f, g), min(j, k));
    vec3 high = max(max(f, g), max(j, k));
    gl_FragColor = vec4(clamp(sum / total, low, high), 1.0);
}
#endif

