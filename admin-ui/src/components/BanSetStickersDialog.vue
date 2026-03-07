<template>
    <v-dialog max-width="500" v-model="dialog" fullscreen scrollable>
        <template v-slot:activator="{ props: activatorProps }">
            <v-btn v-bind="activatorProps" color="surface-variant" text="Open Dialog" variant="flat"></v-btn>
        </template>

        <template v-slot:default="{ isActive }">
            <v-card title="Dialog">
                <v-card-text>
                    {{ error }}
                    <ban-stickers-group :refetch="refetch" :stickers="stickers ?? []" />
                </v-card-text>

                <v-card-actions>
                    <v-spacer></v-spacer>

                    <!-- TODO: actually implement this -->
                    <v-btn  @click="banSet">Ban Set</v-btn>
                    <v-btn text="Close Dialog" @click="isActive.value = false"></v-btn>
                </v-card-actions>
            </v-card>
        </template>
    </v-dialog>
</template>

<script setup lang="ts">
import { useFetch } from '@vueuse/core';
import { computed, ref, watch } from 'vue';
import BanStickersGroup from './BanStickersGroup.vue';

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

const banSet = async () => {
    const { data, error } = await useFetch(`/api/sets/${props.setId}/ban`).post()
    if (error.value) {
        alert(error.value);
    }
    refetch()
}

</script>
