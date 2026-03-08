<template>
                    <div class="d-flex">
                        <div class="flex-0-1-0 flex-grow-1">
                            {{ selected }}
                            <v-btn @click="selectRandom(20)">select 20 at random</v-btn>
                            <v-btn @click="selectRandom(0)">select 0</v-btn>
                            <v-btn @click="selectRandom(sortedStickers.length)">select all</v-btn>
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
                    <v-btn @click="ban">Ban Selection ({{ selected.length }})</v-btn>
                    {{ message }}
</template>

<script setup lang="ts">
import { useFetch } from '@vueuse/core';
import { computed, ref } from 'vue';

const selected = ref<string[]>([]);

const message = ref("");

const props = defineProps<{
    stickers: StickerPub[],
    refetch: () => void,
}>()

interface StickerPub {
    id: string,
    set_id: string,
}

const sortedStickers = computed(() => props.stickers.toSorted((a, b) => {
    const aInc = selected.value.includes(a.id);
    const bInc = selected.value.includes(b.id);
    if (aInc && bInc) return 0;
    if (aInc) return -1;
    if (bInc) return 1;
    return 0;
}))

const selectRandom = (count: number) => {
    selected.value = props.stickers.toSorted(() => Math.random() - 0.5).slice(0, count).map(s => s.id);
}

const ban = async () => {
    for (const stickerId of selected.value) {
        const { data, error } = await useFetch(`/api/stickers/${stickerId}/ban`).post({
          clip_max_match_distance: 0.7 // TODO: dont hardcode
        })
        if (error.value) {
            alert(error.value);
        }
    }
    message.value = `banned ${selected.value.length} stickers`
    props.refetch()
}

</script>
