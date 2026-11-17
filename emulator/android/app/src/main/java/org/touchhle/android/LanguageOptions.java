/* SPDX-License-Identifier: MPL-2.0 */
package org.touchhle.android;

import android.content.Context;
import android.content.SharedPreferences;
import android.view.View;
import android.widget.*;
import java.util.List;

/** Language of the imported game, independent of the launcher UI language. */
final class LanguageOptions {
    private static final String[] CODES = {"en", "de", "fr", "es", "it", "ja", "system"};
    private static final String[] LABELS = {
        "English", "Deutsch", "Français", "Español", "Italiano", "日本語",
        "System language",
    };
    static final String CONTROL_TAG = "hunter-game-language";

    private LanguageOptions() {}

    static String selectedLanguage(Context context) {
        String code = preferences(context).getString("code", "en");
        for (String supported : CODES) if (supported.equals(code)) return code;
        return "en";
    }

    static void addControls(Context context, LinearLayout root) {
        TextView label = new TextView(context);
        label.setText("Game language (applies on next launch)");
        root.addView(label);
        Spinner language = new Spinner(context);
        language.setTag(CONTROL_TAG);
        language.setContentDescription("Game language");
        language.setAdapter(new ArrayAdapter<>(context, android.R.layout.simple_spinner_dropdown_item, LABELS));
        String selected = selectedLanguage(context);
        for (int i = 0; i < CODES.length; i++) if (CODES[i].equals(selected)) language.setSelection(i);
        language.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener() {
            @Override public void onItemSelected(AdapterView<?> parent, View view, int position, long id) {
                preferences(context).edit().putString("code", CODES[position]).apply();
            }
            @Override public void onNothingSelected(AdapterView<?> parent) {}
        });
        root.addView(language);
    }

    static void appendArguments(Context context, List<String> arguments) {
        String code = selectedLanguage(context);
        if (!"system".equals(code)) arguments.add("--preferred-languages=" + code);
    }

    private static SharedPreferences preferences(Context context) {
        return context.getSharedPreferences("language", Context.MODE_PRIVATE);
    }
}
