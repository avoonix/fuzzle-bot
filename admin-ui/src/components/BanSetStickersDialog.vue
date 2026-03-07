<template>
    <v-dialog max-width="500" v-model="dialog" fullscreen scrollable>
        <template v-slot:activator="{ props: activatorProps }">
            <v-btn v-bind="activatorProps" color="surface-variant" text="Open Dialog" variant="flat"></v-btn>
        </template>

        <template v-slot:default="{ isActive }">
            <v-card title="Dialog">
                <v-card-text>
                    {{ error }}
                    <div class="d-flex">
                        <div class="flex-0-1-0 flex-grow-1">
                            {{ selected }}
                            <v-btn @click="selectRandom(20)">select 20 at random</v-btn>
                            <v-item-group multiple v-model="selected">
                                <v-container>
                                    <v-row>
                                        <v-col v-for="sticker of sortedStickers" :key="sticker.id" cols="12" md="4">
                                            <v-item v-slot="{ isSelected, toggle }" :value="sticker.id">
                                                <v-btn  width="auto" height="auto" @click="toggle" :color="isSelected? 'primary':''">
                                                    <img :src="`/files/stickers/${sticker.id}/thumbnail.png`"
                                                        loading="lazy" width="128" height="128" />
                                                </v-btn>
                                            </v-item>
                                        </v-col>
                                    </v-row>
                                </v-container>
                            </v-item-group>
                        </div>
                        <div class="flex-0-1-0 flex-grow-1">
                            TODO: display similar stickers that would be banned
                        </div>
                    </div>
                </v-card-text>

                <v-card-actions>
                    <v-spacer></v-spacer>

                    <!-- TODO: actually implement this -->
                    <v-btn  @click="ban(true)">Ban Selection ({{ selected.length }}) + Set</v-btn>
                    <v-btn @click="ban(false)">Ban Selection ({{ selected.length }})</v-btn>
                    <v-btn text="Close Dialog" @click="isActive.value = false"></v-btn>
                </v-card-actions>
            </v-card>
        </template>
    </v-dialog>
</template>

<script setup lang="ts">
import { useFetch } from '@vueuse/core';
import { computed, ref, watch } from 'vue';

const selected = ref<string[]>([]);

const dialog = ref(false);

const props = defineProps<{
    setId: string
}>()

interface StickerPub {
    id: string,
    set_id: string,
}

const url = computed(() => `/api/sets/${props.setId}/stickers`)

const { data: stickers, error, execute: refetch } = useFetch(url, { refetch: true, updateDataOnError: true, immediate: false }).json<StickerPub[]>()

watch(dialog, () => refetch())

const sortedStickers = computed(() => (stickers.value??[]).toSorted((a, b) => {
    const aInc = selected.value.includes(a.id);
    const bInc = selected.value.includes(b.id);
    if (aInc && bInc) return 0;
    if (aInc) return -1;
    if (bInc) return 1;
    return 0;
}))

const selectRandom = (count: number) => {
    selected.value = (stickers.value ?? []).toSorted(() => Math.random() - 0.5).slice(0, count).map(s => s.id);
}

const ban = async (banSet: boolean) => {
    for (const stickerId of selected.value) {
        const { data, error } = await useFetch(`/api/stickers/${stickerId}/ban`).post({
          clip_max_match_distance: 0.7 // TODO: dont hardcode
        })
        if (error.value) {
            alert(error.value);
        }
    }
    if (banSet) {
        const { data, error } = await useFetch(`/api/sets/${props.setId}/ban`).post()
        if (error.value) {
            alert(error.value);
        }
    }
    refetch()
}

</script>
