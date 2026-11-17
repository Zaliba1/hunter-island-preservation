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
        {"--hunter-test-save", "Testspielstand erstellen / verwenden (separate Kopie)"},
        {"--hunter-level1-save", "Neues Spiel: alle Monster auf Level 1, ohne Zusatzboni"},
        {"--hunter-triple99-save", "Neues Spiel: alle Monster je 3-mal, Level 99, Rang S"},
    };

    private static final String[][] PROFILES = {
        {"--hunter-test-save", "pockettrend.hunterisland-test-v1"},
        {"--hunter-level1-save", "pockettrend.hunterisland-level1-v1"},
        {"--hunter-triple99-save", "pockettrend.hunterisland-triple99-v1"},
    };

    private ModOptions() {}

    static String selectedProfile(Context context) {
        SharedPreferences p=preferences(context);
        String selected=IpaInspector.BUNDLE;
        for (String[] profile : PROFILES) if(p.getBoolean(profile[0],false)) {
            if(!selected.equals(IpaInspector.BUNDLE))throw new IllegalStateException("Bitte nur eine Testsammlung wählen");
            selected=profile[1];
        }
        return selected;
    }

    static void requireTestingSeed(Context context) throws java.io.IOException {
        String selected=selectedProfile(context);
        if (!selected.equals("pockettrend.hunterisland-test-v1")) return;
        java.io.File external=context.getExternalFilesDir(null);
        if(external==null) throw new java.io.IOException("Spielstand-Speicher nicht verfügbar");
        java.io.File parent=new java.io.File(external,"touchHLE_sandbox");
        if(new java.io.File(parent,selected+"/test-profile.json").isFile()) return;
        java.io.File[] slots=new java.io.File(parent,IpaInspector.BUNDLE+"/Documents").listFiles();
        if(slots!=null) for(java.io.File slot:slots) {
            if(!slot.isDirectory() || !slot.getName().matches("saves[0-9]+")) continue;
            java.io.File[] saves=slot.listFiles();
            if(saves!=null) for(java.io.File save:saves) if(save.isFile() && save.getName().matches("sv_[0-9]+")) return;
        }
        throw new java.io.IOException("Bitte zuerst ohne Testspielstand-Haken ein normales Spiel starten und speichern. Danach kann die separate Testkopie erstellt werden.");
    }

    static void addControls(Context context, LinearLayout root) {
        TextView heading = new TextView(context);
        heading.setText("Optionale Mods (ab dem nächsten Spielstart)");
        root.addView(heading);
        SharedPreferences preferences = preferences(context);
        java.util.Map<String,CheckBox> boxes=new java.util.HashMap<>();
        for (String[] option : OPTIONS) {
            String flag = option[0];
            CheckBox checkbox = new CheckBox(context);
            checkbox.setText(option[1]);
            checkbox.setChecked(preferences.getBoolean(flag, false));
            boxes.put(flag,checkbox);
            checkbox.setOnCheckedChangeListener((button, checked) -> {
                if(checked) for(String[] profile:PROFILES) if(profile[0].equals(flag)) {
                    for(String[] other:PROFILES) if(!other[0].equals(flag)) {
                        preferences.edit().putBoolean(other[0],false).apply();
                        if(boxes.containsKey(other[0]))boxes.get(other[0]).setChecked(false);
                    }
                }
                preferences.edit().putBoolean(flag,checked).apply();
            });
            root.addView(checkbox);
        }
        TextView explanation = new TextView(context);
        explanation.setText("Vollsammlung: Kopie des Normalstands. Level 1 / 3-mal Level 99: neues Spiel ohne Zusatzboni. Jede Sammlung hat eigene Saves. Alle Sammlungshaken aus = normaler Spielstand.");
        root.addView(explanation);
    }

    static void appendArguments(Context context, List<String> arguments) {
        selectedProfile(context);
        SharedPreferences preferences = preferences(context);
        for (String[] option : OPTIONS) {
            if (preferences.getBoolean(option[0], false)) arguments.add(option[0]);
        }
    }

    private static SharedPreferences preferences(Context context) {
        return context.getSharedPreferences("mods", Context.MODE_PRIVATE);
    }
}
