# SPDX-License-Identifier: MPL-2.0
"""Compile expedition content into Hunter Island's quest, chat, POI and dungeon formats."""
from copy import deepcopy
from collections import defaultdict
from itertools import combinations

def event(kind, **args):
    return {'class':kind+'GameFlowEvent','arguments':args}

def where(p):
    return 'over-%dx%d' % tuple(p)

def conjunction(conditions):
    result={}
    for clause in reversed([c for c in conditions if c]):
        branch=deepcopy(clause)
        tail=branch
        while 'andCondition' in tail:
            tail=tail['andCondition']
        if result:tail['andCondition']=result
        result=branch
    return result

def reward(kind,**args):
    return {'data':{'type':'DestroyOnPickup.'+kind,'arguments':args},'spawnType':'fixed','chance':1}

def treasure(*items):
    return {'levelScalor':1,'items':list(items)}

def native_monster(entry):
    return {'type':entry['monsterId'],'levelScalor':0,'extraJobArguments':{'level':entry['level']},
            'extraLivingArguments':{'capturable':False}}

def fixed_battle(formation, background, on_win):
    encounter={'chanceToAppear':1,'breeder':{'type':'Normal','arguments':{'blocksRetreat':True,'allowCapture':False}},
               'monsters':[native_monster(m) for m in formation],'winEvent':on_win}
    if on_win['class']=='CompleteQuestGameFlowEvent':encounter['questKey']=on_win['arguments']['questName']
    # Breeder.stats are multipliers used on the whole opposing team in retail.
    first=formation[0]
    scale={'MaxHP':first.get('vitality',1),'Attack':first.get('force',1),'Magic':first.get('force',1)}
    if any(v!=1 for v in scale.values()):encounter['breeder']['arguments']['stats']=scale
    return event('StartBattle',combatBackground=background,monsterPopulator={
        'class':'DefaultMonsterPopulator','arguments':{'levelScalor':1,'fixedEncounters':[encounter]}})

PORTRAITS={'Karen':'Priestess','Charles':'FusionMaster','Albert':'FusionMaster','Werkmeister':'Soldier',
           'Schmied':'Soldier','Töpferin':'Priestess'}
DUNGEONS={
    'over-126x73':('HiTakranDrain','CaveCombat.jpg',30,['Wartungsgang','Kristallnische','Geröllhalle','Absetzbecken']),
    'over-16x43':('HiTowerWorks','SpaceShipCombat.jpg',60,['Wachtor','Teststeg','Schmelzofen','Montagehalle','Schrottgraben','Kalibrierkammer','Entladungsraum']),
    'over-24x16':('HiWinterPass','SnowCaveCombat.jpg',48,['Wachhaus','Quelle','Eisnest','Wächterkammer','Windgang','Frostkammer','Eissee']),
}
# Existing dungeon floors retain their original population and story events.
INTERIORS={
    'over-33x71':('WozanCave',{'Kalter Seitenbach':2}),
    'over-34x11':('RyudouCave',{'Kristallgang':6}),
    'over-123x110':('ArborunDungeon',{'Moosstümpfe':1}),
    'over-101x37':('MountRakken',{'Ascheweide':1,'Steinpferch':2,'Abbaukante':3,'Felsspalten':4,'Lavastollen':5}),
    'over-98x65':('SarkorCave',{'Wachhof':1,'Grabgewölbe':4}),
    'over-87x100':('HeorotDungeon',{'Wachhof':1,'Gewölbe':3}),
}
INTERIOR_BACKGROUNDS = {'WozanCave':'WaterCaveCombat.jpg','RyudouCave':'SnowCaveCombat.jpg',
                        'ArborunDungeon':'ForestCombat.jpg','MountRakken':'FireCaveCombat.jpg',
                        'SarkorCave':'CaveCombat.jpg','HeorotDungeon':'DungeonCombat.jpg'}
# Each entry lists the milestones needed beyond the main dungeon entrance.
FLOOR_GATES={
    'Kristallnische':('bohrer',),'Geröllhalle':('bohrer','Aurandal.WinFinalRound'),
    'Absetzbecken':('bohrer','Aurandal.SearchForTheScale'),
    'Schmelzofen':('werk',),'Montagehalle':('werk',),'Schrottgraben':('werk',),
    'Kalibrierkammer':('+kreise',),'Entladungsraum':('+kreise',),
    'Eisnest':('passofen',),'Wächterkammer':('+eisrezept',),
    'Windgang':('+ewigeseis',),'Frostkammer':('+ewigeseis',),'Eissee':('+ewigeseis',),
}
# Speakers who can identify a sanctuary once its habitat is open.
HABITAT_GUIDES = [
    ('over-39x83', 'Karen', 'Blacklion01', '{6.5,8}', 'Nördlich von Windon mündet ein kalter Seitenbach in den Wozan. Schau dort nach Bluecub.'),
    ('over-92x90', 'Garjes', 'Fireelasmotherium01', '{6.5,6}', 'Am Hang ist die Kante blank gescheuert. Copperhorn kommt dort zum Scharren her.'),
    ('over-92x90', 'Garjes', 'Icetriceratops01', '{6.5,6}', 'In der kalten Mulde hinter Heorot liegen die Abdrücke von Iceratops.'),
    ('over-89x112', 'Karzan', 'Firecaracal01', '{12.5,8}', 'Am alten Kochfeuer liegt noch Glut. Charcal wartet am Waldrand, bis wir verschwunden sind.'),
    ('over-103x129', 'Cardens', 'Shisalion01', '{10.5,8}', 'Such am Waldtor nach den kleinen Pfotenabdrücken von Shisapup.'),
]

class Campaign:
    def __init__(self,config,resources,catalogs,load):
        self.cfg,self.r,self.catalogs,self.load=config,resources,catalogs,load
        self.modified={'Quests','Chats','Towns','Overworld','VsRewards'}
        self.strings={};self.quests={q['id']:q for q in config['quests']}
        self.points={};self.original_event_counts={};self.catalog_entries=defaultdict(list);self.new_dungeons={}
        for name in catalogs:
            for area in resources[name].values():
                for p in area:
                    if p.startswith('over-'):self.catalog_entries[p].append((name,area))
        for ns in ['Quests','Chats']:
            if 'Expedition' in self.r[ns]:raise ValueError('Expedition namespace already exists')
            self.r[ns]['Expedition']={}
    def code(self,q,i):return 'Hi'+q['id'].capitalize()+str(i+1)
    def key(self,q,i):return 'Expedition.'+self.code(q,i)
    def gate(self,ids):
        out=[]
        for k in ids:
            if k=='epilogue':out.extend([{'finishedQuests':['Priory.DefeatTiamat']},{'chats':'EndGame.InfiniteDungeon'}])
            elif k=='kamiwyrm':out.append({'chats':'Expedition.HasFusedKamiwyrm'})
            else:
                q=self.quests.get(k)
                out.append({'finishedQuests':[self.key(q,len(q['steps'])-1) if q else k]})
        return conjunction(out)
    def point(self,p):
        if p not in self.points:
            if p not in self.r:
                if not self.catalog_entries[p]:raise ValueError('Unregistered POI '+p)
                self.r[p]=deepcopy(self.catalog_entries[p][0][1][p])
            self.points[p]=self.r[p]
            self.original_event_counts[p]=len(self.r[p].get('arguments',{}).get('enterEvents',[]))
        return self.points[p]
    def record(self,key,text):
        if key in self.strings and self.strings[key]!=text:raise ValueError('Duplicate text '+key)
        self.strings[key]=text
        return key
    def chat(self,key,npc,portrait,text,after=None):
        self.r['Chats']['Expedition'][key]={'images':[portrait],'messages':[self.record(key+'ChatKey','&@'+npc+'&@'+text)]}
        args={'chatKey':'Expedition.'+key}
        if after:args['nextEvent']=after
        return event('Chat',**args)
    def prompt(self,key,title,message,yes,no=None):
        args={'titleKey':self.record(key+'Title',title),'messageKey':self.record(key+'Question',message),'yesEvent':yes}
        if no:args['noEvent']=no
        return event('YesNoQuestion',**args)
    def actions(self,place,room=''):
        if room and place in DUNGEONS:
            name,_,_,rooms=DUNGEONS[place]
            return self.r[name]['populatorData'][str(1+rooms.index(room))].setdefault('enterEvents',[])
        if room and place in INTERIORS:
            name,rooms=INTERIORS[place]
            self.load([name])
            self.modified.add(name)
            return self.r[name]['populatorData'][str(rooms[room])].setdefault('enterEvents',[])
        return self.point(place).setdefault('arguments',{}).setdefault('enterEvents',[])
    def add_action(self,point,room,action,condition):
        action['arguments']['conditions']=condition
        self.actions(point,room).append(action)
    def quest_strings(self,code,title,detail,progress):
        for k,v in [('Quest'+code+'TitleKey',title),('Quest'+code+'ProgressKey',progress+' (%d/%d)'),
                    ('Quest'+code+'DescriptionChatKey',detail),('SummaryQuest'+code+'DescriptionChatKey',detail),(code+'QuestCompleteChatKey',title+' abgeschlossen.')]:
            self.record(k,v)
    def field_population(self, place):
        # Some standalone POI plists contain a generic early-game population.
        # The regional overworld catalogue supplies the populated tile at runtime.
        entries = self.catalog_entries[place]
        point = next(area[place] for _, area in entries if 'encounters' in area[place]['arguments'])
        encounter = point['arguments']['encounters']['table']['1']['monsterPopulator']
        if isinstance(encounter, str):
            name = encounter.removesuffix('.plist')
            self.load([name])
            encounter = self.r[name]
        return encounter['arguments']

    def floor_population(self, name, floor):
        self.load([name])
        floors = self.r[name]['populatorData']
        # A retail floor with only events or exits inherits the last population.
        for number in range(floor, 0, -1):
            population = floors.get(str(number), {}).get('monsterPopulator')
            if population and population['arguments'].get('types'):
                return population['arguments']
        raise ValueError(f'{name} floor {floor} has no regional population')

    def expedition_floor_types(self, name, room):
        if name == 'HiTakranDrain':
            if room == 'Absetzbecken':
                return [{'type': m, 'chance': w} for m,w in
                        [('Slime02',100),('Wallmonster01',80),('Rockdragon01',20)]]
            if room == 'Wartungsgang':
                return deepcopy(self.floor_population('TunnelB', 1)['types'])
            anchor = 'over-128x81'
        elif name == 'HiTowerWorks':
            anchor = 'over-40x47'
        elif name == 'HiWinterPass':
            anchor = 'over-23x20'
        else:
            raise ValueError('Unknown expedition dungeon ' + name)
        return deepcopy(self.field_population(anchor)['types'])

    def habitat_population(self, h):
        place, room = h['place'], h['room']
        if place in DUNGEONS:
            name, background, _, rooms = DUNGEONS[place]
            floor = rooms.index(room) + 1
        elif place in INTERIORS:
            name, rooms = INTERIORS[place]
            background = INTERIOR_BACKGROUNDS[name]
            floor = rooms[room]
        else:
            anchor = next((where(b['anchor']) for b in self.cfg['branches'] if where(b['end']) == place), place)
            population = self.field_population(anchor)
            terrain = self.point(anchor)['type'].removeprefix('Outdoor.')
            return population['types'], population['levelScalor'], terrain+'Combat.jpg'
        population = self.floor_population(name, floor)
        creation = self.r[name]['creationData']
        level = creation['monsterLevelBase'] + (floor-1)*creation['monsterLevelFloorScalor']
        return population['types'], level, background

    def install_map(self):
        # Reuse each retail door and its return destination after the story door closes.
        for place, dungeon, milestone in (
            ('over-123x110', 'ArborunDungeon.plist', 'pflanzrinne'),
            ('over-87x100', 'HeorotDungeon.plist', 'Heorot.CaptureTheKing'),
        ):
            buildings = self.r['Towns'][place]['buildings']
            original, = (b for b in buildings if b.get('class') == 'ZoneChangeMenuItem'
                         and b['arguments']['zoneChange']['arguments'].get('dungeon', {}).get('arguments') == dungeon)
            return_door = deepcopy(original)
            return_door.pop('enterEvents', None)
            return_door['conditions'] = self.gate([milestone])
            if dungeon == 'HeorotDungeon.plist':
                return_door['iconSpriteFile'] = 'eventIcon2.png'
            buildings.append(return_door)
        links=self.r['Overworld']['contentProviderArguments'].setdefault('connections',[])
        for b in self.cfg['branches']:
            anchor,end=where(b['anchor']),where(b['end'])
            if self.catalog_entries[end] or end in self.r:raise ValueError('Occupied '+end)
            self.point(anchor)
            child=deepcopy(self.catalog_entries[anchor][0][1][anchor])
            if end in DUNGEONS:
                name,bg,level,rooms=DUNGEONS[end]
                child=deepcopy(self.r['over-38x97']);args=child['arguments'];args['instanceName']=name
                args['translationKey']=name+'NameKey'
                args['zoneChange']['arguments']['dungeon']['arguments']=name+'.plist'
                args['zoneChange']['arguments']['dungeon']['type'] = {'HiTakranDrain':'TiledCave','HiTowerWorks':'TiledSpaceship','HiWinterPass':'TiledCaveSnow'}[name]
                cave=deepcopy(self.r['GrayrockCave']);cave['instanceName']=name;cave['translationKey']=name+'NameKey';cave.pop('catchQuestData',None)
                cave['creationData'].update(lastFloor=len(rooms),monsterLevelBase=level,monsterLevelFloorScalor=1,savePointEvery=2)
                template=cave['populatorData']['1'];entrance=deepcopy(template['zoneChanges'][0]);entrance['arguments']['targetZoneChange']=end
                floors={}
                for i,room in enumerate(rooms,1):
                    floor=deepcopy(template);pop=floor['monsterPopulator']['arguments']
                    pop.pop('fixedEncounters',None)
                    pop['types'] = self.expedition_floor_types(name, room)
                    floor['zoneChanges']=([entrance] if i==1 else ['Up'])+(['Down'] if i<len(rooms) else [])
                    floor['enterEvents']=[]
                    for gate in FLOOR_GATES.get(room,()):
                        quest=self.quests.get(gate.lstrip('+'))
                        if gate.startswith('+'):
                            first=self.key(quest,0)
                            locked=conjunction([{'finishedQuests':['!'+first]},{'inProgressQuests':['!'+first]}])
                        else:
                            needed=self.key(quest,len(quest['steps'])-1) if quest else gate
                            locked={'finishedQuests':['!'+needed]}
                        key='HiGate'+name+str(i)+str(len(floor['enterEvents']))
                        hint=self.chat(key,'Wache','Soldier.png','Der Durchgang nach '+room+' ist noch gesperrt. Ich begleite dich zum Eingang.',
                                       event('MoveToWorldMap',world='Overworld',poi=anchor))
                        self.r['Chats']['Expedition'][key]['alwaysShow']=True
                        hint['arguments']['conditions']=locked
                        floor['enterEvents'].append(hint)
                    floors[str(i)]=floor
                cave['populatorData']=floors;self.r[name]=cave;self.modified.add(name)
                self.record(name+'NameKey',b['name'])
            else:
                child['arguments'].pop('enterEvents',None);child['arguments'].pop('zoneChange',None)
                # A copied shore must never replay the original tile's scripted quest battles.
                for row in child['arguments']['encounters']['table'].values():
                    population = row['monsterPopulator']
                    if isinstance(population, str):
                        resource = population.removesuffix('.plist')
                        self.load([resource])
                        population = self.r[resource]
                    if 'fixedEncounters' in population['arguments']:
                        row['monsterPopulator'] = deepcopy(population)
                        row['monsterPopulator']['arguments'].pop('fixedEncounters')
            name,area=self.catalog_entries[anchor][0];area[end]=child;self.catalog_entries[end].append((name,area))
            self.r[end]=child;self.points[end]=child;self.modified.update([name,end])
            links.append({'type':'Connection','arguments':{'poi1':anchor,'poi2':end}})
            first=self.key(self.quests[b['quest']],0)
            locked=conjunction([{'finishedQuests':['!'+first]},{'inProgressQuests':['!'+first]}])
            if b['quest']=='schleppkorb':
                locked=conjunction([locked,{'inProgressQuests':['!Expedition.HiHeimkehr1']},{'finishedQuests':['!Expedition.HiHeimkehr1']}])
            if b['quest']=='geleit':
                early=self.quests['leuchtfeuer']
                not_ready={'finishedQuests':['!'+self.key(early,len(early['steps'])-1)],
                           'orCondition':{'finishedQuests':['!Quests.DarandalPirates']}}
                locked=conjunction([locked,not_ready])
            back=event('MoveToWorldMap',world='Overworld',poi=anchor)
            warning=self.chat('HiClosed'+b['quest'].capitalize(),'Jäger','male_01.png','Der Zugang zu '+b['name']+' ist noch versperrt.',back)
            self.r['Chats']['Expedition']['HiClosed'+b['quest'].capitalize()]['alwaysShow']=True
            warning['arguments']['conditions']=locked;child['arguments'].setdefault('enterEvents',[]).insert(0,warning)
            if end in DUNGEONS:
                name=DUNGEONS[end][0];self.r[name]['populatorData']['1']['enterEvents'].insert(0,deepcopy(warning))
    def offer(self,q,portrait):
        cond=self.gate(q['requires'])
        if q['offer'] in self.r['Towns']:
            t=self.r['Towns'][q['offer']]
            tavern=next((b for b in t['buildings'] if b.get('class')=='TavernMenuItem'),None)
            if tavern:
                tavern['arguments'].setdefault('quests',[]).append(self.key(q,0));return
        gate=conjunction([cond,{'inProgressQuests':['!'+self.key(q,i) for i in range(len(q['steps']))]},
                               {'finishedQuests':['!'+self.key(q,0)]}])
        offer=event('AddQuest',questName=self.key(q,0),questGiver=portrait,imageFilename='eventIcon2.png')
        if q['offer'] in self.r['Towns']:
            t=self.r['Towns'][q['offer']];index=sum(b.get('hiOffer',False) for b in t['buildings'])
            t['buildings'].append({'class':'GameFlowEventsMenuItem','position':'{'+str(5.4+index)+',6.4}',
                'iconSpriteFile':'eventIcon2.png','conditions':gate,'enterEvents':[offer],'hiOffer':True})
        else:self.add_action(q['offer'],'',offer,gate)
    def contextual_greeting(self, q, code, portrait, greeting):
        quest = {
            'schleppkorb':'Quests.DefeatPirates', 'kiesbank':'Quests.DefendDock',
            'windwache':'Quests.DestroyBridge', 'hang':'Quests.EscapedFusionArkadion',
        }.get(q['id'])
        if not quest:
            return self.chat(code+'Offer', q['npc'], portrait, greeting)
        response = None
        if q['id'] == 'hang':
            alternate = greeting
            greeting = 'Reidia hat uns den Hang gezeigt. Hilfst du mir, die vier Wurzeln freizulegen?'
            response = self.chat(code+'Reidia', 'Reidia', 'Reidia.png', q['alternateDialogue'])
        elif q['id'] == 'schleppkorb':
            alternate = q['alternateDialogue']
        else:
            prefix = {'windwache':'Die Brücke kennst du schon. Die Pfeifen dort oben hast du wohl noch nie gehört.',
                      'kiesbank':'Seit wir das Dock wiederhaben, komme ich jeden Morgen hier vorbei.'}
            alternate = prefix[q['id']]+' '+greeting
        after = self.chat(code+'OfferAfter', q['npc'], portrait, alternate, response)
        after['arguments']['conditions'] = self.gate([quest])
        return [after, self.chat(code+'Offer', q['npc'], portrait, greeting)]

    def install_guides(self):
        habitats = {site['species']:site for site in self.cfg['habitats']}
        guides = defaultdict(list)
        for place, npc, species, position, text in HABITAT_GUIDES:
            key = 'HiHint'+species
            chat = self.chat(key, npc, PORTRAITS.get(npc,npc)+'.png', text)
            self.r['Chats']['Expedition'][key]['alwaysShow'] = True
            condition = self.gate(habitats[species]['requires'])
            guides[place, position].append((chat, condition))
        for (place, position), hints in guides.items():
            # Garjes' two tracks unlock together. Show both on one visit.
            gate = hints[0][1]
            assert all(condition == gate for _, condition in hints)
            for (chat, _), (following, _) in zip(hints,hints[1:]):
                chat['arguments']['nextEvent'] = following
            self.r['Towns'][place]['buildings'].append({'class':'GameFlowEventsMenuItem',
                'position':position,'iconSpriteFile':'eventIcon2.png',
                'conditions':gate,'enterEvents':[hints[0][0]]})

    def town_action(self, place, position, action, condition):
        self.r['Towns'][place]['buildings'].append({'class':'GameFlowEventsMenuItem',
            'position':position,'iconSpriteFile':'eventIcon2.png','conditions':deepcopy(condition),'enterEvents':[action]})

    def scene(self, code, lines, after):
        portraits={'Späherin':'Soldier.png','Junger Fänger':'TournamentNPC2.png','Du':'empty.png'}
        images=[]; messages=[]
        for i,(speaker,text) in enumerate(lines,1):
            images.append(portraits.get(speaker,PORTRAITS.get(speaker,speaker)+'.png'))
            messages.append(self.record(code+'Line'+str(i)+'ChatKey','&@'+speaker+'&@'+text))
        self.r['Chats']['Expedition'][code+'Scene']={'images':images,'messages':messages,'alwaysShow':True}
        return event('Chat',chatKey='Expedition.'+code+'Scene',nextEvent=after)

    def install_homecoming(self):
        duel=self.cfg['friendlyDuel']
        win=self.chat('HiFriendlyWin','Junger Fänger','TournamentNPC2.png',duel['ending'])
        self.r['Chats']['Expedition']['HiFriendlyWin']['alwaysShow']=True
        fight=fixed_battle(duel['formation'],duel['background'],win)
        prompt=self.prompt('HiFriendly',duel['title'],duel['question'],fight)
        self.town_action(duel['place'],duel['position'],prompt,{'finishedQuests':['Expedition.HiHeimkehr1']})
        memory=self.chat('HiHomeMemory','Charles','FusionMaster.png',self.cfg['registerMemory'])
        self.r['Chats']['Expedition']['HiHomeMemory']['alwaysShow']=True
        self.town_action('over-25x20',self.quests['heimkehr']['steps'][-1]['position'],memory,self.gate(['heimkehr']))

    def install_quest(self,q):
        portrait=PORTRAITS.get(q['npc'],q['npc'])+'.png'
        trial_names={'blockade':('HiConvoyRestartQuestion','den Ersatzbojen am Außenriff'),
                     'ewigeseis':('HiFrostRestartQuestion','der ersten Etappe im Windgang')}
        trial=trial_names.get(q['id'])
        if trial:
            self.record(trial[0],'Deine Mannschaft hat sich geändert. Mit dieser Aufstellung bei '+trial[1]+' neu beginnen? Wähle Nein, um später mit der ursprünglichen Mannschaft weiterzumachen.')
        for i,step in enumerate(q['steps']):
            key,code=self.key(q,i),self.code(q,i);final=i==len(q['steps'])-1
            room=step.get('room','');place=step['place'];scene=step['action']
            battle_orders = {
                ('wurzel', 0): 'Sieh auf die dunklen Risse in der Rinde. Wenn ein Gegner dort trifft, kosten die Sporen dein Archedion zusätzlich acht Prozent seiner vollen KP. Mit „Runde überspringen“ bringt ihr euch für drei Gegnerzüge in Deckung.',
                ('wurzel', 1): 'Hörst du das Geröll? Bei jedem dritten Gegnerzug rutscht ein getroffener Gefährte nach hinten. Ohne Ersatz kostet ihn das 120 TU. Mit „Runde überspringen“ schützt du die Mannschaft für drei Gegnerzüge.',
                ('wurzel', 2): 'Sichere ich den Graben, kämpft ihr bei den Sporen. Sichere ich den Wurzelpfad, müsst ihr das Geröll abfangen. Die Deckung schützt euch auf beiden Wegen.',
                ('ewigeseis', 2): 'Hör auf das Eis. Beim ersten Knacken holt Cryowyrm aus. Jeder zweite Gegnerzug hat doppelte Angriffsstärke. Mit „Runde überspringen“ nimmst du für drei Gegnerzüge Deckung. Überstehe den ersten Ansturm, dann kannst du ihn besiegen.',
                ('eisrezept', 1): 'Siehst du den Reif auf dem ersten Wächter? Sein Schild lässt nur ein Drittel der Angriffsstärke durch. Nach deinem zweiten Zug springt es auf den zweiten Wächter, nach deinem vierten auf den dritten. Ziele auf einen ungeschützten Wächter.',
                ('blockade', 1): 'Beim vierten ungedeckten Gegnerzug sinkt unser Kahn. Mit „Runde überspringen“ schützt du ihn für die nächsten drei. Nach sechs Gegnerzügen ist die Besatzung in Sicherheit. Vertreibe dann die übrigen Angreifer.',
                ('abnahme', 2): 'Der vierte ungedeckte Gegnerzug trifft den Werkmeister tödlich. Mit „Runde überspringen“ deckst du ihn für die nächsten drei Gegnerzüge. Besiege die Angreifer, solange er noch steht.',
            }
            orders = battle_orders.get((q['id'], i))
            destination = [self.cfg['directions'][place]]
            if room and place in DUNGEONS:
                destination.append('Etage ' + str(DUNGEONS[place][3].index(room) + 1))
            elif room and place in INTERIORS:
                destination.append('Etage ' + str(INTERIORS[place][1][room]))
            if room:
                destination.append(room)
            destination.append(scene + ' ' + orders if orders else scene)
            self.quest_strings(code, q['title'], ' · '.join(destination), scene)
            if step.get('lines') and not final:
                self.strings[code+'QuestCompleteChatKey']=q['steps'][i+1]['action']+'.'
            # Royal road and puzzle interactions have their own completion event at the site.
            # Only encounters that always require combat need a recorded victory.
            required_wins=int(bool(step.get('formation')) and q['id']!='aussaat')
            args={'npc':portrait,'quantity':required_wins,'challengeLevel':step.get('level',1),'hasCustomSummary':True}
            if i==0:
                greeting=q['dialogue']
                if trial:greeting+=' Bleib für alle drei Gefechte bei denselben Archedions und derselben Reihenfolge. Mit einer neuen Mannschaft beginnt die Prüfung von vorn.'
                args['conditions']=self.gate(q['requires']);args['startEvent']=self.contextual_greeting(q,code,portrait,greeting)
            if orders:
                instructions=self.chat(code+'Orders',q['npc'],portrait,orders)
                if i==0:args['startEvent']['arguments']['nextEvent']=instructions
                else:args['startEvent']=instructions
            if final:args['endEvent']=self.chat(code+'End',q['npc'],portrait,q['ending'])
            if 'recipe' in step:args['treasureData']=treasure(reward('Recipe',combineName=step['recipe']))
            self.r['Quests']['Expedition'][code]={'class':'WinBattleQuest','arguments':args}
            finish=event('CompleteQuest',questName=key,questGiver=portrait,rewardLast=True)
            if not final:finish['arguments']['nextEvent']=event('AddQuest',questName=self.key(q,i+1),questGiver=portrait)
            act=finish
            if step.get('formation'):
                bg=DUNGEONS.get(place,(None,None))[1] or {'over-125x108':'ForestCombat.jpg','over-117x47':'BeachCombat.jpg','over-38x74':'BeachCombat.jpg','over-101x37':'FireCaveCombat.jpg','over-98x65':'DungeonCombat.jpg'}.get(place,'FieldCombat.jpg')
                act=fixed_battle(step['formation'],step.get('background',bg),finish)
                if q['id']=='aussaat':
                    act=self.prompt(code+'Water',q['title'],'Das Wasser zuerst auf die zweite Lichtung leiten?',finish,act)
                if step.get('alternative'):
                    act=self.prompt(code+'Route',q['title'],'Soll Reidia den Graben sichern?',act,fixed_battle(step['alternative'],bg,finish))
            if step.get('clue'):
                hint,answers,right=step['clue'];wrong=next(a for n,a in enumerate(answers) if n!=right)
                act=self.prompt(code+'Clue',hint,answers[right]+'?  Nein: '+wrong,act,
                    self.chat(code+'Retry',q['npc'],portrait,'Schau dir die Spuren noch einmal an.'))
            if step.get('lines'):act=self.scene(code,step['lines'],act)
            prompt=self.prompt(code+'Act',q['title'],scene+'?',act)
            ready={'inProgressQuests':[key]}
            if step.get('position'):
                prompt['arguments']['conditions']=ready
                self.town_action(place,step['position'],prompt,ready)
            else:self.add_action(place,room,prompt,ready)
        self.offer(q,portrait)
    def install_habitat(self,h):
        site=h['species'];key='HiSearch'+site
        ambient, ambient_level, bg = self.habitat_population(h)
        low,high=h['levels'];m={'type':site,'levelScalor':0,'extraJobArguments':{'level':low},'extraLivingArguments':{'capturable':True}}
        endgame=bool(set(h['requires'])&{'wurzel','blockade','ewigeseis'})
        pop={'class':'DefaultMonsterPopulator','arguments':{'minMonstersPerPack':1,'maxMonstersPerPack':1,
             'levelScalor':ambient_level,'types':deepcopy(ambient),
             'fixedEncounters':[{'chanceToAppear':1 if endgame else 0.3,'breeder':{'type':'Normal','arguments':{'allowCapture':True}},'monsters':[m]}]}}
        encounter = pop['arguments']['fixedEncounters'][0]
        variants = []
        for level in range(low, high+1):
            fixed = deepcopy(encounter)
            fixed['chanceToAppear'] = 1
            fixed['monsters'][0]['extraJobArguments']['level'] = level
            variants.append([fixed])
        pop['expeditionHabitat'] = {'missCounter':'Expedition.HabitatMisses.'+site,
                                    'guaranteed':endgame, 'levelEncounters':variants}
        fight=event('StartBattle',combatBackground=bg,monsterPopulator=pop)
        prompt=self.prompt(key,h['room'],'Die Spur hier verfolgen?',fight)
        return prompt
    def install_searches(self):
        groups = defaultdict(list)
        for habitat in self.cfg['habitats']:
            place, room = habitat['place'], habitat['room']
            # Retail runs one entry action per visit. Open-air landmarks on the
            # same tile therefore need an explicit path through their choices.
            groups[place, room if place in DUNGEONS or place in INTERIORS else ''].append(habitat)
        for (place, room), sites in groups.items():
            prompts = {site['species']: self.install_habitat(site) for site in sites}
            used_gates = set()
            for size in range(len(sites), 0, -1):
                for choices in combinations(sites, size):
                    needs = tuple(sorted({q for h in choices for q in h['requires']}))
                    if needs in used_gates:
                        continue
                    used_gates.add(needs)
                    menu = None
                    for site in reversed(choices):
                        prompt = deepcopy(prompts[site['species']])
                        prompt['arguments']['conditions'] = self.gate(site['requires'])
                        if menu:
                            prompt['arguments']['noEvent'] = menu
                        menu = prompt
                    self.add_action(place, room, menu, self.gate(needs))

    def install(self):
        self.install_map()
        for q in self.cfg['quests']:self.install_quest(q)
        self.install_searches()
        self.install_guides()
        self.install_homecoming()
        self.install_rewards()
        self.install_nursery()
        self.install_egg_pools()
        for p,value in self.points.items():
            if p not in self.original_event_counts:
                continue  # New points already share their newly created catalog record.
            additions=value.get('arguments',{}).get('enterEvents',[])[self.original_event_counts[p]:]
            if additions:
                self.modified.add(p)
                for cat,area in self.catalog_entries[p]:
                    area[p].setdefault('arguments',{}).setdefault('enterEvents',[]).extend(deepcopy(additions))
                    self.modified.add(cat)
        for t in self.r['Towns'].values():
            for b in t['buildings']:b.pop('hiOffer',None)
    def install_egg_pools(self):
        # The retail shop and reward items can each supply a local set of generators.
        self.load(['GameSettings','Items'])
        settings=self.r['GameSettings']
        locked={s['recipe'] for q in self.cfg['quests'] for s in q['steps'] if 'recipe' in s}
        locked.update(('Crystaldragon01','Crystaldragon02'))
        if not locked-{'Crystaldragon01'} <= set(settings['hiddenRecipes']):
            raise ValueError('A quest recipe would already be shown in the fusion shop')
        # The slot reserved exclusively for finished fusions keeps its original
        # quantity, tier, sale price and repeat policy. Only its species list changes.
        gold=settings['eggTable']['defaultSpin']
        alternatives=list(dict.fromkeys(gold[0]['arguments']['monsters']+gold[1]['arguments']['monsters']))
        if len(alternatives)!=8 or set(alternatives)&locked:
            raise ValueError('Unexpected native starter and dragon egg lists')
        def apply(node):
            if isinstance(node,list):
                for item in node:apply(item)
            elif isinstance(node,dict):
                kind=node.get('class')
                if kind=='MonsterListEggSlotGenerator':
                    args=node['arguments']
                    if locked.intersection(args['monsters']):
                        args['monsters']=[m for m in args['monsters'] if m not in locked] or alternatives.copy()
                elif kind=='RandomMonsterEggSlotGenerator':
                    args=node['arguments']
                    args['excludeList']=list(dict.fromkeys(args.get('excludeList',[])+sorted(locked)))
                else:
                    for item in node.values():apply(item)
        for name in ('GameSettings','Items'):
            apply(self.r[name])
            self.modified.add(name)

    def install_rewards(self):
        self.record('ExpeditionOrblingSafeTitleKey','Orbling bleibt bei dir')
        self.record('ExpeditionOrblingSafeMessageKey','Du hast dieses Orbling für eine besondere Leistung erhalten. Hebe es für die Fusion bei Charles auf.')
        orb=reward('MonsterJoin',shortType='Crystaldragon01',jobLevel=1,tier=5)
        self.r['VsRewards'][:]=[v for v in self.r['VsRewards'] if v.get('reward',{}).get('arguments',{}).get('shortType')!='Crystaldragon01'
                                and v.get('reward',{}).get('arguments',{}).get('combineName')!='Crystaldragon02']
        # Each reward is a single retail GiveItems event. Its unique identifier
        # is persisted by the game even when an existing hunter revisits the site.
        deeds=[('Story','over-26x111','Priory.DefeatTiamat'),('Dragons','over-42x47','Quests.HuntPowerfulDragon3'),
               ('Forest','over-125x108','wurzel'),('Convoy','over-117x47','blockade'),('Frost','over-24x16','ewigeseis')]
        speakers={
            'Story':('Casper','Casper.png','Als du aus Orlen aufgebrochen bist, hat dieses Orbling in meine Hand gepasst. Jetzt, nach Tiamat, soll es mit dir reisen.','Besiege Tiamat und kehre zu Casper nach Orlen zurück.'),
            'Dragons':('Beatrice','Priestess.png','Drei Drachenjagden. Ich habe die letzte Seite deines Berichts selbst versiegelt. Die Order vertraut dir dieses Orbling an.','Beatrice erwartet dich nach der dritten Drachenjagd in Reijin.'),
            'Forest':('Reidia','Reidia.png','Zwischen den freigelegten Wurzeln hat sich etwas bewegt. Sieh mal, wie fest sich das Orbling an deinem Ärmel hält.','Hilf Reidia in Arborun bei der letzten Wurzel.'),
            'Convoy':('Mardu','Mardu.png','Als wir den Anker hochzogen, saß es zwischen den Kettengliedern. Hier an Deck findet es keinen Halt. Nimm du es.','Brich mit Mardu die Blockade von Darandal.'),
            'Frost':('Passwache','Soldier.png','Beim Auftauen ist ein Orbling aus der Frostkammer gekrochen. Jetzt folgt es jedem Schritt deiner Stiefel.','Schließe am Wärterpfad bei Yelan „Unter dem ewigen Eis“ ab.'),
        }
        deliveries=[]
        hints=[]
        for name,place,gate in deeds:
            speaker,sprite,line,hint=speakers[name]
            receipt='Expedition.HiDeed'+name
            signed=self.chat('HiDeed'+name,speaker,sprite,line)
            give=event('GiveItems',uniqueIdentifier='HiOrbling'+name,appendLocationToIdentifier=False,
                       treasureData=treasure(orb),nextEvent=signed)
            ready=conjunction([self.gate([gate]),{'chats':'!'+receipt}])
            if place in self.r['Towns']:
                position='{3.5,7}' if name=='Story' else '{7.5,8}'
                self.r['Towns'][place]['buildings'].append({'class':'GameFlowEventsMenuItem','position':position,
                    'iconSpriteFile':'eventIcon2.png','conditions':deepcopy(ready),'enterEvents':[deepcopy(give)]})
            self.add_action(place,'',deepcopy(give),deepcopy(ready))
            if gate in self.quests or name=='Dragons':
                if name=='Dragons':
                    quest=self.r['Quests']['Quests']['HuntPowerfulDragon3']
                else:
                    q=self.quests[gate];quest=self.r['Quests']['Expedition'][self.code(q,len(q['steps'])-1)]
                ending=quest['arguments']['endEvent']['arguments']
                if 'nextEvent' in ending:raise ValueError('Original quest ending changed: '+gate)
                closing=deepcopy(give)
                closing['arguments']['conditions']={'chats':'!'+receipt}
                ending['nextEvent']=closing
            give['arguments']['conditions']=ready
            deliveries.append(give)
            message=self.chat('HiDeedHint'+name,'Charles','FusionMaster.png',hint)
            self.r['Chats']['Expedition']['HiDeedHint'+name]['alwaysShow']=True
            milestone=self.gate([gate])['finishedQuests'][0]
            message['arguments']['conditions']=conjunction([{'finishedQuests':['!'+milestone]},{'chats':'!'+receipt}])
            hints.append(message)
        t=self.r['Towns']['over-25x20']
        forms={species for family,group in self.r['Monsters'].items() if family!='RootMonster' for species in group}
        if len(forms)!=360 or 'Crystaldragon02' not in forms:
            raise ValueError('Expected the complete 2.02 Monsterpedia')
        register='Expedition.Formenregister/'+'|'.join(sorted(forms-{'Crystaldragon02'}))
        self.record(register+'TypeKey','Arten')
        for count in [250,300,359]:
            code='HiRegister'+str(count);self.quest_strings(code,'Charles’ Monsterpedia',str(count)+' Arten eintragen','Arten eintragen')
            # CollectMonstersQuest supplies target, localized type, progress and target.
            self.strings['Quest'+code+'ProgressKey']='Sammle %d %@ (%d/%d)'
            items=([orb] if count==250 else [orb,reward('Recipe',combineName='Crystaldragon02')] if count==359 else [])
            args={'challengeLevel':1,'num':count,'typeFilter':register,'treasureData':treasure(*items)}
            if count==300:
                args.pop('treasureData')
                self.strings['Quest'+code+'ProgressKey']='Brutstation: %d %@ (%d/%d)'
            if count==359:args['conditions']=conjunction([{'finishedQuests':['Expedition.HiRegister250']}]+[{'chats':'Expedition.HiDeed'+name} for name,_,_ in deeds])
            self.r['Quests']['Expedition'][code]={'class':'CollectMonstersQuest','arguments':args}
        template=next(b for b in self.r['Towns']['over-26x111']['buildings'] if b.get('class')=='ChallengeQuestMenuItem')
        intro=self.record('HiCharlesRegisterIntro','&@Charles&@Bei 250 Arten vertraue ich dir ein Orbling an. Bei 300 öffnet die Brutstation in Orlen. Bei 359 kannst du hier die Briefe abholen. Jede Entwicklung und jeder Inselgeist zählt.')
        board=deepcopy(template);board['position']='{6.5,8}';board['arguments'].update(questGiver='FusionMaster.png',
            quests=['Expedition.HiRegister250','Expedition.HiRegister300','Expedition.HiRegister359'])
        board['arguments']['chatKey']=intro
        complete=[{'finishedQuests':['Expedition.HiRegister250']}]+[{'chats':'Expedition.HiDeed'+name} for name,_,_ in deeds]
        missing={}
        for condition in reversed(complete):
            field,value=next(iter(condition.items()))
            missing={field:['!'+value[0]] if isinstance(value,list) else '!'+value, **({'orCondition':missing} if missing else {})}
        ordinary=deepcopy(board)
        ordinary['conditions']=missing
        ordinary['arguments']['quests'].pop()
        review='HiRegisterReview'
        self.quest_strings(review,'Charles prüft die Übergaben','Ausstehende Orbling in Yelan abholen','Übergaben prüfen')
        self.strings['Quest'+review+'ProgressKey']='Briefe: %d %@ (%d/%d)'
        self.r['Quests']['Expedition'][review]={'class':'CollectMonstersQuest','arguments':{
            'num':359,'challengeLevel':1,'typeFilter':register}}
        ordinary['arguments']['quests'].append('Expedition.'+review)
        board['conditions']=conjunction(complete)
        t['buildings'].extend([ordinary,board])
        t['buildings'].append({'class':'GameFlowEventsMenuItem','position':'{7.5,10.5}',
            'iconSpriteFile':'eventIcon2.png','conditions':conjunction([{'finishedQuests':['Expedition.'+review]},missing]),
            'enterEvents':deliveries+hints})
    def install_nursery(self):
        next_offer=None
        for egg in reversed(self.cfg['eggCommissions']):
            key='HiNest'+egg['id']
            guards={
                'Air':('Windpenguin03','FieldCombat.jpg'),'Earth':('Rhyno02','CaveCombat.jpg'),
                'Water':('Icebear03','SnowCaveCombat.jpg'),'Fire':('Firespinosaurus01','FireCaveCombat.jpg'),
                'Arcane':('Roboscorpion01','SpaceShipCombat.jpg'),'Life':('Grasssquirrel01','ForestCombat.jpg'),
                'Death':('Mummy01','DungeonCombat.jpg')}
            protector,background=guards[egg['element']]
            habitat=next((h for h in self.cfg['habitats'] if h['species']==egg['id']),None)
            if habitat:raise ValueError('A nursery target already has a new hunting ground')
            pop={'class':'DefaultMonsterPopulator','arguments':{'levelScalor':1,'fixedEncounters':[{
                'chanceToAppear':1,'breeder':{'type':'Normal','arguments':{'blocksRetreat':True,'allowCapture':False}},
                'monsters':[native_monster({'monsterId':protector,'level':75})],
                'treasureData':treasure(reward('MonsterJoin',shortType=egg['id'],jobLevel=1,tier=4))}]}}
            fight=event('StartBattle',combatBackground=background,monsterPopulator=pop)
            next_offer=self.prompt(key,'Brutstation in Orlen',egg['name']+' vor den Nesträubern schützen? Nein zeigt das nächste Gelege.',fight,next_offer)
        self.r['Towns']['over-26x111']['buildings'].append({'class':'GameFlowEventsMenuItem','position':'{6.0,7.2}',
            'iconSpriteFile':'eventIcon2.png','conditions':conjunction([self.gate(['epilogue']),{'finishedQuests':['Expedition.HiRegister300']}]),
            'enterEvents':[next_offer]})
