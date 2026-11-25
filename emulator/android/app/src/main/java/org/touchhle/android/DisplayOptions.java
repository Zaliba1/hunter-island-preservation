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
    private static final String[] FILTERS = {"bilinear", "anime4k", "fsrcnnx"};
    private static final String[] FILTER_LABELS = {"Bilinear", "Anime4K CNN x2 S", "FSRCNNX x2"};
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
        preset.setText("Recommended display settings · Bilinear + local HD artwork + 2× internal resolution");
        preset.setTag("recommended-view");
        root.addView(preset);
        TextView filterLabel = new TextView(context);
        filterLabel.setText("Output filter");
        advanced.addView(filterLabel);
        Spinner filter = new Spinner(context);
        filter.setAdapter(new ArrayAdapter<>(context, android.R.layout.simple_spinner_dropdown_item, FILTER_LABELS));
        int index = java.util.Arrays.asList(FILTERS).indexOf(preferences.getString("upscale", FILTERS[0]));
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
        internal2x.setChecked(preferences.getBoolean("internal2x", true));
        advanced.addView(internal2x);

        CheckBox hd = new CheckBox(context);
        hd.setTag("hunter-hd-textures");
        hd.setText("Local HD artwork (monsters 4×, characters, scenes and rarity stars 2×)");
        hd.setChecked(preferences.getBoolean("hdTextures", true));

        advanced.addView(hd);
        Runnable sync = () -> preset.setChecked(filter.getSelectedItemPosition() == 0 && internal2x.isChecked() && hd.isChecked() && !integerScaling.isChecked());
        sync.run();
        internal2x.setOnCheckedChangeListener((button, checked) -> {
            preferences.edit().putBoolean("internal2x", checked).apply();
            sync.run();
        });
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
            filter.setSelection(0);
            preferences.edit().putString("upscale", FILTERS[0]).apply();
            internal2x.setChecked(enabled);
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
        String filter = preferences.getString("upscale", FILTERS[0]);
        if (!"bilinear".equals(filter) && java.util.Arrays.asList(FILTERS).contains(filter)) arguments.add("--upscale="+filter);
        String profile = preferences.getString("family", PROFILES[0]);
        arguments.add("ipad".equals(profile) ? "--device-family=ipad" : "--device-family=iphone");
        if ("iphone-retina".equals(profile)) arguments.add("--experimental-retina");
        if (preferences.getBoolean("integerScaling", false)) arguments.add("--integer-scaling");
        if (preferences.getBoolean("internal2x", true)) arguments.add("--scale-hack=2");
        if (preferences.getBoolean("hdTextures", true)) arguments.add("--hunter-hd-textures");
    }

    private static SharedPreferences preferences(Context context) {
        SharedPreferences prefs = context.getSharedPreferences("display", Context.MODE_PRIVATE);
        if (!prefs.getBoolean("retinaPresetV2", false)) {
            // The previous launcher stored the effective settings, without a separate preset switch.
            String filter = prefs.getString("upscale", "anime4k");
            boolean hd = prefs.getBoolean("hdTextures", !prefs.contains("upscale"));
            boolean doubled = prefs.getBoolean("internal2x", false);
            boolean previousDefault = PROFILES[0].equals(prefs.getString("family", PROFILES[0]))
                    && "anime4k".equals(filter) && hd && !doubled && !prefs.getBoolean("integerScaling", false);
            if (previousDefault || !java.util.Arrays.asList(FILTERS).contains(filter)) {
                filter = FILTERS[0];
            }
            prefs.edit().putString("upscale", filter).putBoolean("hdTextures", hd)
                    .putBoolean("internal2x", doubled || previousDefault)
                    .putBoolean("retinaPresetV2", true).apply();
        }
        return prefs;
    }
}
