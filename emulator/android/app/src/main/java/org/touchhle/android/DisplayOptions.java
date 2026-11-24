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
    private static final String[] PROFILES = {"iphone-retina", "iphone", "ipad"};
    private static final String[] LABELS = {
        "iPhone Retina · 960 × 640 · experimental", "iPhone · 480 × 320", "iPad · 1024 × 768",
    };
    static final String INTEGER_SCALING_TAG = "hunter-integer-scaling";

    private DisplayOptions() {}

    static void addControls(Context context, LinearLayout root) {
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

        CheckBox integerScaling = new CheckBox(context);
        integerScaling.setText("Integer scaling (sharper pixels, larger borders)");
        integerScaling.setTag(INTEGER_SCALING_TAG);
        integerScaling.setChecked(preferences.getBoolean("integerScaling", false));
        integerScaling.setOnCheckedChangeListener((button, checked) ->
            preferences.edit().putBoolean("integerScaling", checked).apply());
        root.addView(integerScaling);

        CheckBox hd = new CheckBox(context);
        hd.setTag("hunter-hd-textures");
        hd.setText("Generate HD artwork locally (monsters 4×, selected artwork 2×)");
        hd.setChecked(preferences.getBoolean("hdTextures", false));
        hd.setOnCheckedChangeListener((button, checked) ->
            preferences.edit().putBoolean("hdTextures", checked).apply());
        root.addView(hd);
    }

    static void appendArguments(Context context, List<String> arguments) {
        SharedPreferences preferences = preferences(context);
        String profile = preferences.getString("family", PROFILES[0]);
        arguments.add("ipad".equals(profile) ? "--device-family=ipad" : "--device-family=iphone");
        if ("iphone-retina".equals(profile)) arguments.add("--experimental-retina");
        if (preferences.getBoolean("integerScaling", false)) arguments.add("--integer-scaling");
        if (preferences.getBoolean("hdTextures", false)) arguments.add("--hunter-hd-textures");
    }

    private static SharedPreferences preferences(Context context) {
        return context.getSharedPreferences("display", Context.MODE_PRIVATE);
    }
}
