/* SPDX-License-Identifier: MPL-2.0 */
package org.touchhle.android;

import android.content.Context;
import android.content.SharedPreferences;
import android.view.View;
import android.widget.AdapterView;
import android.widget.ArrayAdapter;
import android.widget.CheckBox;
import android.widget.LinearLayout;
import android.widget.Spinner;
import android.widget.TextView;
import java.util.List;

final class DisplayOptions {
    private static final String[] FILTERS = {"bilinear", "fsr", "fsr-sharpened", "anime4k", "fsrcnnx"};
    private static final String[] FILTER_LABELS = {"Bilinear", "FSR 1.2", "FSR 1.2 + RCAS", "Anime4K CNN x2 S", "FSRCNNX x2"};
    private static final String[] PROFILES = {"iphone-retina", "iphone", "ipad"};
    private static final String[] LABELS = {
        "iPhone Retina · 960 × 640 · experimental", "iPhone · 480 × 320", "iPad · 1024 × 768",
    };
    static final String INTEGER_SCALING_TAG = "hunter-integer-scaling";

    private DisplayOptions() {}

    static void addControls(Context context, LinearLayout root, LinearLayout advanced) {
        SharedPreferences preferences = preferences(context);
        TextView label = new TextView(context);
        label.setText("Display mode (applies on next launch)");
        root.addView(label);

        Spinner display = new Spinner(context);
        display.setAdapter(new ArrayAdapter<>(context, android.R.layout.simple_spinner_dropdown_item, LABELS));
        String family = preferences.getString("family", PROFILES[0]);
        for (int i = 0; i < PROFILES.length; i++) {
            if (PROFILES[i].equals(family)) display.setSelection(i);
        }
        display.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener() {
            @Override public void onItemSelected(AdapterView<?> parent, View view, int position, long id) {
                preferences.edit().putString("family", PROFILES[position]).apply();
            }
            @Override public void onNothingSelected(AdapterView<?> parent) {}
        });
        root.addView(display);
        CheckBox preset = new CheckBox(context);
        preset.setText("Recommended view settings · Anime4K + local HD artwork");
        preset.setTag("recommended-view");
        root.addView(preset);
        boolean fresh = !preferences.contains("upscale") && !preferences.contains("hdTextures");
        if (fresh) preferences.edit().putBoolean("hdTextures", true).apply();
        TextView filterLabel = new TextView(context);
        filterLabel.setText("Output filter");
        advanced.addView(filterLabel);
        Spinner filter = new Spinner(context);
        filter.setAdapter(new ArrayAdapter<>(context, android.R.layout.simple_spinner_dropdown_item, FILTER_LABELS));
        int index = java.util.Arrays.asList(FILTERS).indexOf(preferences.getString("upscale", "anime4k"));
        filter.setSelection(Math.max(0,index));
        advanced.addView(filter);

        CheckBox integerScaling = new CheckBox(context);
        integerScaling.setText("Integer scaling (sharper pixels, larger borders)");
        integerScaling.setTag(INTEGER_SCALING_TAG);
        integerScaling.setChecked(preferences.getBoolean("integerScaling", false));

        advanced.addView(integerScaling);

        CheckBox internal2x = new CheckBox(context);
        internal2x.setText("2× internal resolution");
        internal2x.setTag("hunter-internal-2x");
        internal2x.setChecked(preferences.getBoolean("internal2x", false));
        internal2x.setOnCheckedChangeListener((button, checked) ->
            preferences.edit().putBoolean("internal2x", checked).apply());
        advanced.addView(internal2x);

        CheckBox hd = new CheckBox(context);
        hd.setTag("hunter-hd-textures");
        hd.setText("Local HD artwork (monsters 4×, characters, scenes and rarity stars 2×)");
        hd.setChecked(preferences.getBoolean("hdTextures", fresh));

        advanced.addView(hd);
        Runnable sync = () -> preset.setChecked(filter.getSelectedItemPosition() == 3 && hd.isChecked() && !integerScaling.isChecked());
        sync.run();
        hd.setOnCheckedChangeListener((button, checked) -> {
            preferences.edit().putBoolean("hdTextures", checked).apply();
            sync.run();
        });
        integerScaling.setOnCheckedChangeListener((button, checked) -> {
            preferences.edit().putBoolean("integerScaling", checked).apply();
            sync.run();
        });
        preset.setOnClickListener(v -> {
            boolean enabled = preset.isChecked();
            filter.setSelection(enabled ? 3 : 0);
            preferences.edit().putString("upscale", enabled ? "anime4k" : "bilinear").apply();
            hd.setChecked(enabled);
            if (enabled) integerScaling.setChecked(false);
            sync.run();
        });
        filter.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener() {
            @Override public void onItemSelected(AdapterView<?> parent, View view, int position, long id) {
                preferences.edit().putString("upscale", FILTERS[position]).apply();
                sync.run();
            }
            @Override public void onNothingSelected(AdapterView<?> parent) {}
        });

    }

    static void appendArguments(Context context, List<String> arguments) {
        SharedPreferences preferences = preferences(context);
        String filter = preferences.getString("upscale", "anime4k");
        if (!"bilinear".equals(filter) && java.util.Arrays.asList(FILTERS).contains(filter)) arguments.add("--upscale="+filter);
        String profile = preferences.getString("family", PROFILES[0]);
        arguments.add("ipad".equals(profile) ? "--device-family=ipad" : "--device-family=iphone");
        if ("iphone-retina".equals(profile)) arguments.add("--experimental-retina");
        if (preferences.getBoolean("integerScaling", false)) arguments.add("--integer-scaling");
        if (preferences.getBoolean("internal2x", false)) arguments.add("--scale-hack=2");
        if (preferences.getBoolean("hdTextures", !preferences.contains("upscale"))) arguments.add("--hunter-hd-textures");
    }

    private static SharedPreferences preferences(Context context) {
        return context.getSharedPreferences("display", Context.MODE_PRIVATE);
    }
}
