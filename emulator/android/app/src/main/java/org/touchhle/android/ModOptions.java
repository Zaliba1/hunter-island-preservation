/* SPDX-License-Identifier: MPL-2.0 */
package org.touchhle.android;

import android.content.Context;
import android.content.SharedPreferences;
import android.widget.CheckBox;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.util.List;

/** Optional rules for the locally imported Hunter Island game. */
final class ModOptions {
    private static final String[][] OPTIONS = {
        {"--hunter-rare-dragons", "Seltene Startdrachen häufiger finden (5 % pro Wildkampf)"},
        {"--hunter-silver-exchange", "Silber gegen Gold tauschen (50:1)"},
        {"--hunter-no-arena-cooldown", "Kolosseum ohne 12-Stunden-Wartezeit"},
    };

    private ModOptions() {}

    static void addControls(Context context, LinearLayout root) {
        TextView heading = new TextView(context);
        heading.setText("Optionale Mods (ab dem nächsten Spielstart)");
        root.addView(heading);
        SharedPreferences preferences = preferences(context);
        for (String[] option : OPTIONS) {
            String flag = option[0];
            CheckBox checkbox = new CheckBox(context);
            checkbox.setText(option[1]);
            checkbox.setChecked(preferences.getBoolean(flag, false));
            checkbox.setOnCheckedChangeListener((button, checked) ->
                preferences.edit().putBoolean(flag, checked).apply());
            root.addView(checkbox);
        }
    }

    static void appendArguments(Context context, List<String> arguments) {
        SharedPreferences preferences = preferences(context);
        for (String[] option : OPTIONS) {
            if (preferences.getBoolean(option[0], false)) arguments.add(option[0]);
        }
    }

    private static SharedPreferences preferences(Context context) {
        return context.getSharedPreferences("mods", Context.MODE_PRIVATE);
    }
}
