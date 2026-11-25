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
        {"--zalibas-expansion", "Zalibas Expansion"},
        {"--hunter-rare-dragons", "Find rare starter dragons more often (5% per wild battle)"},
        {"--hunter-silver-exchange", "Exchange silver for gold (50:1)"},
        {"--hunter-no-arena-cooldown", "Skip the 12-hour arena wait"},
        {"--hunter-test-save", "Full collection and resources (separate save copy)"},
        {"--hunter-level1-save", "New game with all monsters at level 1"},
        {"--hunter-triple99-save", "New game with 3 of each monster, level 99, rank S"},
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
            if(!selected.equals(IpaInspector.BUNDLE))throw new IllegalStateException("Choose only one testing collection");
            selected=profile[1];
        }
        return selected + (p.getBoolean("--zalibas-expansion",false) ? "-expansion" : "");
    }

    static String campaignProfile(Context context) {
        return IpaInspector.BUNDLE + (preferences(context).getBoolean("--zalibas-expansion", false) ? "-expansion" : "");
    }

    static void requireTestingSeed(Context context) throws java.io.IOException {
        String selected=selectedProfile(context);
        if (!selected.equals("pockettrend.hunterisland-test-v1") && !selected.equals("pockettrend.hunterisland-test-v1-expansion")) return;
        java.io.File external=context.getExternalFilesDir(null);
        if(external==null) throw new java.io.IOException("Save storage is unavailable");
        java.io.File parent=new java.io.File(external,"touchHLE_sandbox");
        if(new java.io.File(parent,selected+"/test-profile.json").isFile()) return;
        java.io.File[] slots=new java.io.File(parent,IpaInspector.BUNDLE+(selected.endsWith("-expansion") ? "-expansion" : "")+"/Documents").listFiles();
        if(slots!=null) for(java.io.File slot:slots) {
            if(!slot.isDirectory() || !slot.getName().matches("saves[0-9]+")) continue;
            java.io.File[] saves=slot.listFiles();
            if(saves!=null) for(java.io.File save:saves) if(save.isFile() && save.getName().matches("sv_[0-9]+")) return;
        }
        if (selected.endsWith("-expansion") && "copy".equals(preferences(context).getString("expansionOrigin", "")) && hasCheckpoint(new java.io.File(parent,IpaInspector.BUNDLE))) return;
        throw new java.io.IOException("Save a normal game once before creating a full collection copy.");
    }

    private static boolean hasCheckpoint(java.io.File profile) {
        java.io.File[] slots = new java.io.File(profile, "Documents").listFiles();
        if (slots == null) return false;
        for (java.io.File slot : slots) {
            if (!slot.getName().matches("saves[0-9]+")) continue;
            java.io.File[] saves = slot.listFiles(file -> file.isFile() && file.getName().matches("sv_[0-9]+"));
            if (saves != null && saves.length > 0) return true;
        }
        return false;
    }
    static void chooseCampaign(Context context, Runnable accept, Runnable cancel) {
        java.io.File external = context.getExternalFilesDir(null);
        if (external == null) { cancel.run(); return; }
        java.io.File parent = new java.io.File(external, "touchHLE_sandbox");
        java.io.File expansion = new java.io.File(parent, IpaInspector.BUNDLE + "-expansion");
        SharedPreferences p = preferences(context);
        if (p.contains("expansionOrigin") || new java.io.File(expansion, ".campaign-origin-v1").exists()
                || hasCheckpoint(expansion) || !hasCheckpoint(new java.io.File(parent, IpaInspector.BUNDLE))) {
            accept.run(); return;
        }
        new android.app.AlertDialog.Builder(context).setTitle("Start Zalibas Expansion")
            .setMessage("Copy your original campaign into a separate Expansion save, or start a new campaign?")
            .setPositiveButton("Copy campaign", (d,w) -> { p.edit().putString("expansionOrigin","copy").apply(); accept.run(); })
            .setNegativeButton("Start new", (d,w) -> { p.edit().putString("expansionOrigin","new").apply(); accept.run(); })
            .setNeutralButton("Cancel", (d,w) -> cancel.run()).setOnCancelListener(d -> cancel.run()).show();
    }

    static void addControls(Context context, LinearLayout root) {
        TextView heading = new TextView(context);
        heading.setText("Optional mods (apply on next launch)");
        root.addView(heading);
        SharedPreferences preferences = preferences(context);
        java.util.Map<String,CheckBox> boxes=new java.util.HashMap<>();
        for (String[] option : OPTIONS) {
            String flag = option[0];
            CheckBox checkbox = new CheckBox(context);
            checkbox.setText(option[1] + (flag.equals("--zalibas-expansion") || flag.equals("--hunter-no-arena-cooldown") ? " · Recommended by the developer" : ""));
            checkbox.setChecked(preferences.getBoolean(flag, false));
            boxes.put(flag,checkbox);
            checkbox.setOnCheckedChangeListener((button, checked) -> {
                if (checked && flag.equals("--zalibas-expansion")) {
                    chooseCampaign(context, () -> preferences.edit().putBoolean(flag, true).apply(), () -> checkbox.setChecked(false));
                    return;
                }
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
        explanation.setText("Each collection has its own saves. Leave all three unchecked to use your normal saves. Level 1 and level 99 starts have no bonus resources.");
        root.addView(explanation);
    }

    static void appendArguments(Context context, List<String> arguments) {
        selectedProfile(context);
        SharedPreferences preferences = preferences(context);
        String origin = preferences.getString("expansionOrigin", "");
        if (preferences.getBoolean("--zalibas-expansion", false) && ("copy".equals(origin) || "new".equals(origin))) {
            arguments.add("--expansion-" + origin + "-campaign");
        }
        for (String[] option : OPTIONS) {
            if (preferences.getBoolean(option[0], false)) arguments.add(option[0]);
        }
    }

    private static SharedPreferences preferences(Context context) {
        return context.getSharedPreferences("mods", Context.MODE_PRIVATE);
    }
}
